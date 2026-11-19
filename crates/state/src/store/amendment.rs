use super::{Store, TaskGuard};
use crate::{
	TaskRecord,
	documents::{publish, read_supported},
	evidence::EffectRecord,
	revision::{self, Revision, WriterUpdate},
};
use workyard_core::{AmendRequest, AmendState, Problem, Task};

pub struct RevisionCheckpoint(Option<Vec<u8>>);

impl AmendState for Store {
	type Checkpoint = RevisionCheckpoint;

	fn begin_amend(
		&self,
		guard: &TaskGuard,
		task: &Task,
		request: &AmendRequest,
	) -> Result<RevisionCheckpoint, Problem> {
		self.verify()?;
		guard.verify()?;
		let anchor: TaskRecord = read_supported(&guard.evidence_path, &[1, 2, 3, 4])?;
		let previous = revision::read(&guard.evidence_path, &anchor, &self.root, &self.state)?;
		let previous = previous
			.map(|_| {
				std::fs::read(revision::path(&guard.evidence_path))
					.map_err(|error| Problem::io("Read amendment checkpoint", error))
			})
			.transpose()?;
		publish(
			&revision::path(&guard.evidence_path),
			&Revision {
				version: 1,
				anchor,
				request: revision::RequestRecord::new(request, &self.state),
				current: EffectRecord {
					version: 1,
					task: task.into(),
					pending: None,
				},
			},
		)?;
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

	fn amend_request(&self, guard: &TaskGuard) -> Result<Option<AmendRequest>, Problem> {
		self.verify()?;
		guard.verify()?;
		let anchor: TaskRecord = read_supported(&guard.evidence_path, &[1, 2, 3, 4])?;
		revision::read(&guard.evidence_path, &anchor, &self.root, &self.state)?
			.map(|revision| revision.request.decode(&self.state))
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
		let anchor: TaskRecord = read_supported(&guard.evidence_path, &[1, 2, 3, 4])?;
		let Some(mut revision) =
			revision::read(&guard.evidence_path, &anchor, &self.root, &self.state)?
		else {
			return Ok(false);
		};
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
