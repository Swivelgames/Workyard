use crate::{Repository, Task};
use std::path::PathBuf;

#[derive(Clone, Debug)]
pub struct RecoveryRecord {
	/// Last published task document, distinct from independent effect evidence.
	pub recorded: Option<Task>,
	/// Existing formats may need a supported migration even when Git is unchanged.
	pub needs_publication: bool,
	pub task: Task,
	pub pending: Option<usize>,
}

#[derive(Clone, Debug)]
pub struct RecoveryObservation {
	pub repository: Repository,
	pub destination: Option<PathBuf>,
	pub operation_in_progress: bool,
}

/// The invocation result is separate from preserved task operation history.
pub struct RecoveryResult {
	pub task: Task,
	pub record: PathBuf,
	pub nothing_to_recover: bool,
}

impl RecoveryRecord {
	/// Effect snapshots may retain older operation provenance after final publication.
	/// Compare reconciled resource facts, not that stale invocation outcome.
	pub(crate) fn matches_published(&self, task: &Task) -> bool {
		!self.needs_publication
			&& self.recorded.as_ref().is_some_and(|recorded| {
				recorded.problem.is_none()
					&& recorded.operation.outcome == crate::OperationOutcome::Succeeded
					&& recorded.participants == task.participants
					&& recorded.lifecycle == task.lifecycle
					&& recorded.removal == task.removal
			})
	}
}
