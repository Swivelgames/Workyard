//! Recovery backups are user-owned once the chosen worktree has been installed.
use super::{Relocation, Selection, conflict};
use crate::{exists, relocation::record::Journal};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use workyard_core::Problem;
use workyard_system::{entry_identity, rename_no_replace};

#[derive(Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(in crate::relocation) enum BackupEndpoint {
	Source,
	Destination,
}

impl BackupEndpoint {
	fn for_direction(undo: bool) -> Self {
		if undo {
			Self::Source
		} else {
			Self::Destination
		}
	}
}

impl Journal {
	pub(in crate::relocation) fn backup_path(
		&self,
		index: usize,
		undo: bool,
		number: usize,
	) -> Result<PathBuf, Problem> {
		let member = &self.participants[index];
		let endpoint = if undo {
			member.original(&self.tasks).destination.decode()?
		} else {
			member.destination.decode()?
		};
		Ok(endpoint.with_file_name(format!("workyard-backup-{}-{index}-{number}", self.id)))
	}

	pub(in crate::relocation) fn retained_backups(&self) -> Result<Vec<PathBuf>, Problem> {
		let mut paths = Vec::new();
		for (index, member) in self.participants.iter().enumerate() {
			for (number, &endpoint) in member.backup_endpoints.iter().enumerate() {
				paths.push(self.backup_path(index, endpoint == BackupEndpoint::Source, number)?);
			}
		}
		Ok(paths)
	}
}

impl Relocation {
	pub fn retained_backups(&self) -> Result<Vec<PathBuf>, Problem> {
		self.journal.retained_backups()
	}

	pub(super) fn retain_backup(
		&mut self,
		index: usize,
		selection: &Selection,
		number: usize,
		displaced: &Path,
	) -> Result<(), Problem> {
		let member = &self.journal.participants[index];
		let endpoint = BackupEndpoint::for_direction(selection.undo);
		if member.backup_endpoints.get(number) == Some(&endpoint) {
			return Ok(());
		}
		if number != member.backup_endpoints.len() {
			return Err(conflict("Invalid recovery backup sequence."));
		}

		let backup = self.journal.backup_path(index, selection.undo, number)?;
		if exists(displaced)? {
			if Some(entry_identity(displaced)?) != selection.displaced_id {
				return Err(conflict("Displaced worktree changed before backup."));
			}
			rename_no_replace(displaced, &backup).map_err(|error| {
				Problem::io(format!("Keep displaced files at {backup:?}"), error)
			})?;
		} else if !exists(&backup)? || Some(entry_identity(&backup)?) != selection.displaced_id {
			return Err(conflict(&format!(
				"Cannot locate the displaced worktree at {backup:?}."
			)));
		}

		self.journal.participants[index]
			.backup_endpoints
			.push(endpoint);
		self.persist()
	}
}
