//! Locate recorded project selections while retaining the original admission evidence.
use crate::{TaskRecord, path::StoredPath};
use std::{
	collections::BTreeMap,
	path::{Path, PathBuf},
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
	let Ok(relative) = original.strip_prefix(previous) else {
		return original.to_owned();
	};
	let moved = workspace.join(relative);
	if moved != original && matches!(moved.try_exists(), Ok(false)) {
		return original.to_owned();
	}
	moved
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
