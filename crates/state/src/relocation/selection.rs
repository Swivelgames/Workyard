//! Decide which verified worktree supplies a recovered participant.
mod backup;
pub(super) use backup::BackupEndpoint;
mod cleanup;
mod decision;
mod install;
mod paths;
mod resume;
mod tree;

use super::{Relocation, conflict};
use crate::{exists, path::StoredPath};
use paths::validate_paths;
use serde::{Deserialize, Serialize};
use std::{
	fs,
	path::{Path, PathBuf},
};
use workyard_core::{Problem, RelocationParticipant};
use workyard_system::entry_identity;

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Selection {
	registered: StoredPath,
	administration: StoredPath,
	registered_id: (u64, u64),
	undo: bool,
	use_registered: bool,
	occupied: bool,
	selected_digest: String,
	first_digest: Option<String>,
	registered_digest: String,
	displaced_pointer: Option<Vec<u8>>,
	retired_entries: Vec<super::record::Content>,
	retiring: bool,
	#[serde(default, skip_serializing_if = "Option::is_none")]
	backup_number: Option<usize>,
	displaced_id: Option<(u64, u64)>,
	stage_id: Option<(u64, u64)>,
	installed: bool,
}

pub(super) fn endpoints(entry: &RelocationParticipant, undo: bool) -> (&Path, &Path) {
	if undo {
		(&entry.original.destination, &entry.destination)
	} else {
		(&entry.destination, &entry.original.destination)
	}
}

impl Selection {
	pub(super) fn require_settled(
		&self,
		journal: &super::record::Journal,
		index: usize,
		undo: bool,
	) -> Result<(), Problem> {
		let [stage, displaced] = self.temporary_paths(journal, index)?;
		if !self.installed || exists(&stage)? || exists(&displaced)? {
			return Err(conflict(
				"Participant file selection has not finished publication.",
			));
		}
		let member = &journal.participants[index];
		let origin = member.original(&journal.tasks).destination.decode()?;
		let location = if undo {
			origin.clone()
		} else {
			member.destination.decode()?
		};
		let relative = origin
			.strip_prefix(journal.old_root.decode()?)
			.map_err(|_| conflict("Invalid recovery endpoint."))?;
		let admitted = journal.content.iter().find(|e| e.path == relative.into());
		let expected = if undo == self.undo {
			self.stage_id.or(self.displaced_id)
		} else {
			admitted.and_then(|e| {
				if undo {
					member.recovered_source.or(e.origin_identity)
				} else {
					member.recovered_destination.or(e.destination_identity)
				}
			})
		};
		if !exists(&location)? || Some(entry_identity(&location)?) != expected {
			return Err(conflict(&format!(
				"Recovery endpoint at {location:?} changed before activation."
			)));
		}
		Ok(())
	}

	pub(super) fn validate(
		&self,
		journal: &super::record::Journal,
		index: usize,
	) -> Result<(), Problem> {
		validate_paths(journal, index, &self.registered()?)
	}

	pub(super) fn scratch_paths(
		&self,
		journal: &super::record::Journal,
		index: usize,
	) -> Result<Vec<PathBuf>, Problem> {
		let mut paths = Vec::new();
		let registered = self.registered()?;
		if (registered.starts_with(journal.old_root.decode()?)
			|| registered.starts_with(journal.new_root.decode()?))
			&& exists(&registered)?
			&& entry_identity(&registered)? == self.registered_id
		{
			paths.push(registered);
		}
		if let Some(number) = self.backup_number {
			let backup = journal.backup_path(index, self.undo, number)?;
			if exists(&backup)? && Some(entry_identity(&backup)?) == self.displaced_id {
				paths.push(backup);
			}
		}
		let [stage, displaced] = self.temporary_paths(journal, index)?;
		for (path, id) in [(stage, self.stage_id), (displaced, self.displaced_id)] {
			if exists(&path)? && id == Some(entry_identity(&path)?) {
				paths.push(path);
			}
		}
		Ok(paths)
	}

	pub(super) fn registered(&self) -> Result<PathBuf, Problem> {
		self.registered.decode()
	}

	pub(super) fn temporary_paths(
		&self,
		journal: &super::record::Journal,
		index: usize,
	) -> Result<[PathBuf; 2], Problem> {
		let p = &journal.participants[index];
		let dest = if self.undo {
			p.original(&journal.tasks).destination.decode()?
		} else {
			p.destination.decode()?
		};
		let parent = dest
			.parent()
			.ok_or_else(|| conflict("Recovery endpoint lacks a parent."))?;
		Ok([
			parent.join(format!(".workyard-{}-{index}.stage", journal.id)),
			parent.join(format!(".workyard-{}-{index}.displaced", journal.id)),
		])
	}
}

impl Relocation {
	pub(super) fn previous_registration(
		&self,
		entry: &RelocationParticipant,
	) -> Result<Option<PathBuf>, Problem> {
		let index = self.member_index(entry)?;
		self.journal.participants[index]
			.selection
			.as_ref()
			.map(Selection::registered)
			.transpose()
	}

	fn member_index(&self, entry: &RelocationParticipant) -> Result<usize, Problem> {
		self.journal
			.participants
			.iter()
			.position(|p| {
				p.task == entry.task && p.destination == entry.destination.as_path().into()
			})
			.ok_or_else(|| conflict("Unknown recovery participant."))
	}

	pub(super) fn save_selection(
		&mut self,
		index: usize,
		selection: &Selection,
	) -> Result<(), Problem> {
		let member = &mut self.journal.participants[index];
		if selection.installed {
			let identity = selection.stage_id.or(selection.displaced_id);
			if selection.undo {
				member.recovered_source = identity;
			} else {
				member.recovered_destination = identity;
			}
		}
		member.selection = Some(selection.clone());
		self.persist()
	}
}

pub(super) fn read_directory(path: &Path) -> Result<fs::Metadata, Problem> {
	let meta =
		fs::symlink_metadata(path).map_err(|e| Problem::io("Inspect recovery directory", e))?;
	if !meta.is_dir() {
		return Err(conflict("Expected a recovery directory."));
	}
	Ok(meta)
}

fn read_pointer(path: &Path) -> Result<Vec<u8>, Problem> {
	let pointer = path.join(".git");
	if !fs::symlink_metadata(&pointer)
		.map_err(|e| Problem::io("Inspect participant pointer", e))?
		.is_file()
	{
		return Err(conflict("Expected a regular participant pointer."));
	}
	fs::read(pointer).map_err(|e| Problem::io("Read participant pointer", e))
}
