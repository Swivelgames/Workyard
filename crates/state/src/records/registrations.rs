//! Locate recorded project selections while retaining the original admission evidence.
use crate::{TaskRecord, path::StoredPath};
use std::{
	collections::BTreeMap,
	path::{Component, Path, PathBuf},
};
use workyard_core::Problem;

impl TaskRecord {
	pub(crate) fn relocated_project_roots(
		&self,
		workspace: &Path,
	) -> Result<BTreeMap<PathBuf, PathBuf>, Problem> {
		let state = self.state_dir.registration()?;
		let previous = state.parent().expect("validated workspace marker");
		let roots = self.participants.iter().flat_map(|p| &p.registration_roots);
		relocated_registrations(roots, previous, workspace)
	}
}

/// Resolve an explicitly supplied project at the current workspace, keeping its
/// previous absolute spelling when the project stayed behind.
pub(crate) fn located_project(original: &Path, previous: &Path, workspace: &Path) -> PathBuf {
	let Some(relative) = workspace_relative(original, previous) else {
		return original.to_owned();
	};
	let moved = workspace.join(relative);
	if moved != original && matches!(moved.try_exists(), Ok(false)) {
		return original.to_owned();
	}
	moved
}

fn workspace_relative<'a>(path: &'a Path, workspace: &Path) -> Option<&'a Path> {
	let relative = path.strip_prefix(workspace).ok();
	#[cfg(windows)]
	let relative = relative.or_else(|| strip_verbatim_workspace(path, workspace));

	relative.filter(|suffix| {
		suffix
			.components()
			.all(|part| matches!(part, Component::CurDir | Component::Normal(_)))
	})
}

#[cfg(windows)]
fn strip_verbatim_workspace<'a>(path: &'a Path, workspace: &Path) -> Option<&'a Path> {
	use std::path::Prefix;

	// Explicit selections can use a DOS or UNC path while the marker uses
	// its canonical verbatim prefix. Compare the volume, then whole components.
	let mut selected = path.components();
	let mut marker = workspace.components();
	let (Component::Prefix(a), Component::Prefix(b)) = (selected.next()?, marker.next()?) else {
		return None;
	};
	let same_volume = match (a.kind(), b.kind()) {
		(Prefix::Disk(a), Prefix::VerbatimDisk(b)) => a == b,
		(Prefix::UNC(a, b), Prefix::VerbatimUNC(c, d)) => a == c && b == d,
		_ => false,
	};
	if !same_volume || !marker.all(|part| selected.next() == Some(part)) {
		return None;
	}
	Some(selected.as_path())
}

pub(crate) fn relocated_registrations<'a>(
	roots: impl Iterator<Item = &'a StoredPath>,
	previous: &Path,
	workspace: &Path,
) -> Result<BTreeMap<PathBuf, PathBuf>, Problem> {
	let mut relocated = BTreeMap::new();
	if previous == workspace {
		return Ok(relocated);
	}

	for stored in roots {
		let original = stored.registration()?;
		let located = located_project(&original, previous, workspace);
		if located != original {
			relocated.insert(original, located);
		}
	}
	Ok(relocated)
}

#[cfg(all(test, windows))]
#[test]
fn windows_selections_must_stay_within_the_recorded_workspace() {
	for (selected, workspace, expected) in [
		(r"C:\old", r"C:\old", Some("")),
		(r"C:\old", r"\\?\C:\old", Some("")),
		(r"C:\old\repo", r"C:\old", Some("repo")),
		(r"C:\old\repo", r"\\?\C:\old", Some("repo")),
		(r"D:\old\repo", r"\\?\C:\old", None),
		(r"C:\older\repo", r"\\?\C:\old", None),
		(r"C:\old\..\outside", r"C:\old", None),
		(r"C:\old\..\outside", r"\\?\C:\old", None),
		(
			r"\\host\share\old\repo",
			r"\\?\UNC\host\share\old",
			Some("repo"),
		),
		(r"\\host\other\old\repo", r"\\?\UNC\host\share\old", None),
	] {
		assert_eq!(
			workspace_relative(Path::new(selected), Path::new(workspace)),
			expected.map(Path::new),
			"selected: {selected:?}, workspace: {workspace:?}"
		);
	}
}
