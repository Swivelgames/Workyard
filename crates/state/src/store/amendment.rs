use super::{Store, TaskGuard};
use crate::{
	TaskRecord,
	documents::publish,
	evidence::EffectRecord,
	revision::{self, Revision, WriterUpdate},
};
use workyard_core::{AmendRequest, AmendState, Problem, RemovalTarget, Task};

pub struct RevisionCheckpoint(Option<Vec<u8>>);

impl AmendState for Store {
	type Checkpoint = RevisionCheckpoint;

	fn staging_available(&self, path: &std::path::Path) -> Result<bool, Problem> {
		match std::fs::symlink_metadata(path) {
			Ok(_) => Ok(false),
			Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
				workyard_core::State::validate_destination(self, path)?;
				Ok(true)
			}
			Err(error) => Err(Problem::io("Inspect replacement staging location", error)),
		}
	}

	fn same_destination(
		&self,
		left: &std::path::Path,
		right: &std::path::Path,
	) -> Result<bool, Problem> {
		if left == right {
			return Ok(true);
		}
		if left.parent() != right.parent() {
			return Ok(false);
		}
		for path in [left, right] {
			match std::fs::symlink_metadata(path) {
				Ok(metadata) if metadata.is_dir() => {}
				Ok(_) => return Ok(false),
				Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(false),
				Err(error) => {
					return Err(Problem::io("Compare participant directory entries", error));
				}
			}
		}
		Ok(workyard_system::entry_identity(left)? == workyard_system::entry_identity(right)?)
	}

	fn begin_amend(
		&self,
		guard: &TaskGuard,
		task: &Task,
		request: &AmendRequest,
		targets: &[RemovalTarget],
	) -> Result<RevisionCheckpoint, Problem> {
		self.verify()?;
		guard.verify()?;
		let anchor = TaskRecord::read(&guard.evidence_path)?;
		let previous = revision::read(&guard.evidence_path, &anchor, &self.root, &self.state)?;
		let removed = previous
			.as_ref()
			.map(|r| r.removed.clone())
			.unwrap_or_default();
		let previous = previous
			.map(|_| {
				std::fs::read(revision::path(&guard.evidence_path))
					.map_err(|error| Problem::io("Read amendment checkpoint", error))
			})
			.transpose()?;
		let revision = Revision {
			version: 1,
			anchor,
			removed,
			request: Some(revision::RequestRecord::new(request, targets, &self.state)),
			current: EffectRecord {
				version: 1,
				task: task.into(),
				pending: None,
			},
		};
		revision.validate_targets()?;
		publish(&revision::path(&guard.evidence_path), &revision)?;
		Ok(RevisionCheckpoint(previous))
	}

	fn unchanged_amend(
		&self,
		guard: &TaskGuard,
		checkpoint: RevisionCheckpoint,
	) -> Result<(), Problem> {
		self.verify()?;
		guard.verify()?;
		let path = revision::path(&guard.evidence_path);
		match checkpoint.0 {
			Some(previous) => crate::documents::publish_bytes(&path, &previous),
			None => std::fs::remove_file(path)
				.map_err(|error| Problem::io("Discard unchanged amendment intent", error)),
		}
	}

	fn removal_targets(&self, guard: &TaskGuard) -> Result<Vec<RemovalTarget>, Problem> {
		self.verify()?;
		guard.verify()?;
		let anchor = TaskRecord::read(&guard.evidence_path)?;
		let revision = revision::read(&guard.evidence_path, &anchor, &self.root, &self.state)?;
		Ok(revision
			.into_iter()
			.filter_map(|revision| revision.request)
			.flat_map(|request| request.remove)
			.map(Into::into)
			.collect())
	}

	fn settle_unlink(
		&self,
		guard: &TaskGuard,
		task: &Task,
		target: &RemovalTarget,
	) -> Result<(), Problem> {
		self.verify()?;
		guard.verify()?;
		let anchor = TaskRecord::read(&guard.evidence_path)?;
		let mut revision = revision::read(&guard.evidence_path, &anchor, &self.root, &self.state)?
			.ok_or_else(|| {
				crate::damage::damaged("Missing membership revision for participant removal.")
			})?;
		revision.settle_unlink(task, target)?;
		publish(&revision::path(&guard.evidence_path), &revision)
	}

	fn amend_request(&self, guard: &TaskGuard) -> Result<Option<AmendRequest>, Problem> {
		self.verify()?;
		guard.verify()?;
		let anchor = TaskRecord::read(&guard.evidence_path)?;
		revision::read(&guard.evidence_path, &anchor, &self.root, &self.state)?
			.and_then(|revision| revision.request)
			.map(|request| request.decode(&self.state))
			.transpose()
	}
}

impl Store {
	pub(super) fn revise(
		&self,
		guard: &TaskGuard,
		task: &Task,
		writers: WriterUpdate,
	) -> Result<bool, Problem> {
		let anchor = TaskRecord::read(&guard.evidence_path)?;
		let Some(mut revision) =
			revision::read(&guard.evidence_path, &anchor, &self.root, &self.state)?
		else {
			return Ok(false);
		};
		revision.removed.retain(|receipt| {
			!task.participants.iter().any(|p| {
				p.worktree_identity.is_some() && p.backing_identity == receipt.backing_identity
			})
		});
		// Retirement owns future recovery. Undo can recreate a worktree with
		// a new administrative identity while previous unlink receipts stay valid.
		if task.removal.is_some() {
			revision.request = None;
		}
		revision.current.task = task.into();
		match writers {
			WriterUpdate::Preserve => {}
			WriterUpdate::Pending(index) => revision.current.pending = Some(index),
			WriterUpdate::Settled => revision.current.pending = None,
		}
		publish(&revision::path(&guard.evidence_path), &revision)?;
		Ok(true)
	}
}
