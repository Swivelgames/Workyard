//! Check live candidate identities and the scope of persisted recovery paths.
use super::{Relocation, conflict, endpoints};
use crate::{canonical_future, exists};
use std::path::Path;
use workyard_core::{Problem, RegisteredCopy, RelocationParticipant};
use workyard_system::entry_identity;

impl Relocation {
	pub(super) fn verify_candidates(
		&self,
		index: usize,
		entry: &RelocationParticipant,
		undo: bool,
		reg: &RegisteredCopy,
	) -> Result<(), Problem> {
		let (dest, alternate) = endpoints(entry, undo);
		for path in [dest, alternate, reg.path.as_path()] {
			if canonical_future(path)? != path {
				return Err(conflict(
					"A recovery path traverses a redirected directory.",
				));
			}
		}
		validate_paths(&self.journal, index, &reg.path)?;
		self.verify_directory_identities(index, entry)
	}

	pub(super) fn verify_directory_identities(
		&self,
		index: usize,
		entry: &RelocationParticipant,
	) -> Result<(), Problem> {
		let (dest, alternate) = endpoints(entry, false);
		let member = &self.journal.participants[index];
		for endpoint in [dest, alternate] {
			if !exists(endpoint)? {
				continue;
			}
			let directory = &entry.original.destination;
			let relative = directory
				.strip_prefix(self.old_root()?)
				.map_err(|_| conflict("Recovery participant is outside the copied workspace."))?;
			let admitted = self
				.journal
				.content
				.iter()
				.find(|item| item.path == relative.into() && item.kind == "directory");
			let expected = admitted.and_then(|item| {
				if endpoint == directory {
					member.recovered_source.or(item.origin_identity)
				} else {
					member.recovered_destination.or(item.destination_identity)
				}
			});
			let actual = Some(entry_identity(endpoint)?);
			let published = member
				.selection
				.as_ref()
				.is_some_and(|s| s.stage_id == actual && endpoint == endpoints(entry, s.undo).0);
			if actual != expected && !published {
				return Err(conflict(&format!(
					"Recovery directory at {endpoint:?} has been replaced."
				)));
			}
		}
		Ok(())
	}
}

pub(super) fn validate_paths(
	journal: &crate::relocation::record::Journal,
	index: usize,
	registered: &Path,
) -> Result<(), Problem> {
	let member = &journal.participants[index];
	let source = member.original(&journal.tasks).destination.decode()?;
	let dest = member.destination.decode()?;
	let root = journal.old_root.decode()?;
	let relative = source
		.strip_prefix(&root)
		.map_err(|_| conflict("Recovery participant is outside the copied workspace."))?;
	if !journal.copy
		|| relative.as_os_str().is_empty()
		|| journal.new_root.decode()?.join(relative) != dest
		|| !registered.is_absolute()
		|| registered.components().any(|part| {
			matches!(
				part,
				std::path::Component::ParentDir | std::path::Component::CurDir
			)
		}) {
		return Err(conflict("Invalid manually moved participant anchors."));
	}
	let overlap = |a: &Path, b: &Path| a.starts_with(b) || b.starts_with(a);
	for state in [journal.old_state.decode()?, journal.new_state.decode()?] {
		if [source.as_path(), dest.as_path(), registered]
			.into_iter()
			.any(|p| overlap(p, &state))
		{
			return Err(conflict("Recovery candidate overlaps workspace state."));
		}
	}
	for (position, member) in journal.participants.iter().enumerate() {
		let participant = member.original(&journal.tasks);
		for path in [
			&participant.source,
			&participant.common_dir,
			&participant.destination,
			&member.destination,
		] {
			let path = path.decode()?;
			if position == index && registered == path && (path == source || path == dest) {
				continue;
			}
			if overlap(registered, &path) {
				return Err(conflict(
					"Manually registered worktree overlaps a participant or backing repository.",
				));
			}
		}
	}
	Ok(())
}
