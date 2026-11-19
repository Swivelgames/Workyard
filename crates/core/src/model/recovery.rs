use crate::{Repository, Task};
use std::{collections::BTreeMap, path::PathBuf};

#[derive(Clone, Debug)]
pub struct RecoveryRecord {
	/// Last published task document, distinct from independent effect evidence.
	pub recorded: Option<Task>,
	/// Existing formats may need a supported migration even when Git is unchanged.
	pub needs_publication: bool,
	pub task: Task,
	pub pending: Option<usize>,
	/// Current locations of admission roots whose recorded workspace moved.
	pub relocated_project_roots: BTreeMap<PathBuf, PathBuf>,
}

#[derive(Clone, Debug)]
pub struct RecoveryObservation {
	pub repository: Repository,
	pub destination: Option<PathBuf>,
	pub operation_in_progress: bool,
}

/// The invocation result is separate from preserved task operation history.
pub struct RecoveryResult {
	pub admitted_roots: Vec<PathBuf>,
	pub operation: crate::Operation,
	pub problem: Option<crate::Problem>,
	pub pull_failures: Vec<crate::PullFailure>,
	pub task: Task,
	pub record: PathBuf,
	pub nothing_to_recover: bool,
}

impl RecoveryRecord {
	/// New revisions can publish current registration paths. Creation evidence
	/// keeps its original selections for comparison against the immutable admission.
	pub(crate) fn into_amendment_task(mut self) -> Task {
		for participant in &mut self.task.participants {
			locate_roots(
				&mut participant.registration_roots,
				&self.relocated_project_roots,
			);
		}
		self.task
	}

	pub(crate) fn newly_admitted_roots(&self, task: &Task) -> Vec<PathBuf> {
		let mut roots = task.admitted_roots(Some(&self.task));
		locate_roots(&mut roots, &self.relocated_project_roots);
		roots
	}

	pub(crate) fn writer_problem(&self, index: usize) -> crate::Problem {
		let participant = &self.task.participants[index];
		let operation = match self.task.operation.continues.or(self.task.operation.kind) {
			Some(crate::OperationKind::Remove) => "the removal operation",
			_ => match participant.phase {
				crate::Phase::Creating => "worktree creation",
				crate::Phase::Pulling => "the pull",
				crate::Phase::Fetching => "the fetch",
				_ => "the task operation",
			},
		};
		crate::Problem::unconfirmed_git_exit(format_args!(
			"{operation} for participant {:?} at {:?}",
			participant.name, participant.destination
		))
	}

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

fn locate_roots(roots: &mut [PathBuf], locations: &BTreeMap<PathBuf, PathBuf>) {
	for root in roots {
		if let Some(located) = locations.get(root) {
			root.clone_from(located);
		}
	}
}
