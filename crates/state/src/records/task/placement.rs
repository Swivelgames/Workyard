//! Relative paths follow the marker; absolute evidence identifies a previous Git link.
use super::{ParticipantRecord, TaskRecord};
use crate::{damage::damaged, path::StoredPath};
use serde::{Deserialize, Serialize};
use std::path::{Component, Path, PathBuf};
use workyard_core::{Participant, PlacementHint, Problem};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LocationRecord {
	source: StoredPath,
	common_dir: StoredPath,
	destination: StoredPath,
	workspace: StoredPath,
	entry: Option<(u64, u64)>,
}

impl LocationRecord {
	pub(super) fn from_participant(p: &Participant, state: &Path) -> Option<Self> {
		if let Some(hint) = &p.placement {
			return Some(Self {
				source: hint.source.as_path().into(),
				common_dir: hint.common_dir.as_path().into(),
				destination: hint.destination.as_path().into(),
				workspace: hint.workspace.as_path().into(),
				entry: hint.worktree_entry,
			});
		}
		p.worktree_identity.as_ref()?;
		Some(Self {
			source: p.source.as_path().into(),
			common_dir: p.common_dir.as_path().into(),
			destination: p.destination.as_path().into(),
			workspace: state.parent().expect("workspace marker").into(),
			entry: workyard_system::entry_identity(&p.destination).ok(),
		})
	}

	pub(super) fn decode(self) -> Result<PlacementHint, Problem> {
		Ok(PlacementHint {
			source: self.source.registration()?,
			common_dir: self.common_dir.registration()?,
			destination: self.destination.registration()?,
			workspace: self.workspace.registration()?,
			worktree_entry: self.entry,
		})
	}
}

pub(super) fn anchored(path: &Path, state: &Path) -> StoredPath {
	let root = state.parent().expect("workspace marker");
	let stored = path.strip_prefix(root).unwrap_or(path);
	if stored.as_os_str().is_empty() {
		Path::new(".").into()
	} else {
		stored.into()
	}
}

pub(super) fn check_path(path: &Path) -> Result<(), Problem> {
	if path.as_os_str().as_encoded_bytes().contains(&0) {
		return Err(damaged(
			"Task record contains an unrepresentable native path (NUL).",
		));
	}
	if path.as_os_str().is_empty()
		|| path.components().any(|c| c == Component::ParentDir)
		|| (!path.is_absolute()
			&& path
				.components()
				.any(|c| matches!(c, Component::Prefix(_) | Component::RootDir)))
	{
		return Err(damaged("Invalid portable Git path."));
	}
	Ok(())
}

pub(super) fn matches_workspace(
	record: &TaskRecord,
	root: &Path,
	state: &Path,
) -> Result<bool, Problem> {
	let prior = record.state_dir.registration()?;
	let tasks = record.tasks_root.decode()?;
	check_path(&tasks)?;
	let previous = prior
		.parent()
		.ok_or_else(|| damaged("Invalid workspace marker."))?;
	if prior.file_name() != state.file_name() {
		return Ok(false);
	}
	if tasks.is_absolute()
		&& let Some(detached) = crate::workspace::detached_tasks_path(state)?
	{
		let workspace = state.parent().expect("workspace marker");
		return Ok(crate::canonical_future(&previous.join(&detached))? == tasks
			&& crate::canonical_future(&workspace.join(detached))? == root);
	}
	let relative = tasks.strip_prefix(previous).unwrap_or(&tasks);
	Ok(!relative.is_absolute()
		&& root
			.strip_prefix(state.parent().expect("workspace marker"))
			.ok() == Some(relative))
}

pub(super) fn participant(
	record: ParticipantRecord,
	previous: &Path,
	state: &Path,
	detached_anchor: Option<(&Path, &Path)>,
) -> Result<Participant, Problem> {
	let old_workspace = previous.parent().expect("validated workspace marker");
	let workspace = state.parent().expect("workspace marker");
	let mut p = Participant::try_from(record)?;
	let old_source = p
		.placement
		.as_ref()
		.map_or_else(|| old_workspace.join(&p.source), |hint| hint.source.clone());
	// Older local records may contain absolute paths. Re-anchor those paths
	// before using the previous location as evidence for a verified link.
	let at_workspace = |path: &Path| {
		if let Some((old, new)) = detached_anchor
			&& path.is_absolute()
			&& let Ok(relative) = path.strip_prefix(old)
		{
			return new.join(relative);
		}
		workspace.join(path.strip_prefix(old_workspace).unwrap_or(path))
	};
	let source = at_workspace(&p.source);
	// Keep a source that remained at the original location after moving just state and tasks.
	p.source = if old_source != source && matches!(source.try_exists(), Ok(false)) {
		old_source
	} else {
		source
	};
	p.common_dir = at_workspace(&p.common_dir);
	p.destination = at_workspace(&p.destination);
	Ok(p)
}

pub(super) fn detached_anchor<'a>(
	record: &TaskRecord,
	previous: &'a Path,
	current: &'a Path,
) -> Result<Option<(&'a Path, &'a Path)>, Problem> {
	let tasks: PathBuf = record.tasks_root.decode()?;
	if previous == current || !tasks.is_absolute() {
		return Ok(None);
	}

	// Absolute placements predate detached layouts. Rebase them only when the
	// current layout explicitly declares the detached relationship.
	if crate::workspace::detached_tasks_path(current)?.is_none() {
		return Ok(None);
	}
	Ok(previous
		.parent()
		.expect("workspace marker")
		.ancestors()
		.zip(current.parent().expect("workspace marker").ancestors())
		.find(|(old, _)| tasks.starts_with(old)))
}
