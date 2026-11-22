use crate::{
	AmendRequest, Control, Participant, Problem, RecoveryGit, RecoveryState, Repository, Task,
};
use std::{future::Future, path::Path};

/// A participant reconciled with its observed Git registration.
pub struct Relationship {
	/// Participant to which this observation or change applies.
	pub participant: Participant,
	/// Whether the Git links require reconciliation.
	pub needs_repair: bool,
}

/// Git operations required to add, move and reconcile participants.
pub trait AmendGit: RecoveryGit + crate::RemovalGit + crate::ObservationGit {
	/// Locate the registered participant and determine whether its links need repair.
	fn relationship(
		&self,
		repository: &Repository,
		participant: &Participant,
		control: &Control,
	) -> impl Future<Output = Result<Relationship, Problem>> + Send;

	/// Repair the relationship for the verified participant.
	fn repair(
		&self,
		repository: &Repository,
		participant: &Participant,
		control: &Control,
	) -> impl Future<Output = Result<(), Problem>> + Send;

	/// Ask Git to move the participant worktree to its destination.
	fn move_worktree(
		&self,
		repository: &Repository,
		participant: &Participant,
		destination: &Path,
		control: &Control,
	) -> impl Future<Output = Result<(), Problem>> + Send;
}

/// Persistent intent and checkpoints needed for a resumable amendment.
pub trait AmendState: RecoveryState {
	/// State saved before amendment effects begin.
	type Checkpoint: Send;
	/// May a private staging name be used without adopting an existing entry?
	fn staging_available(&self, path: &Path) -> Result<bool, Problem>;

	/// Does this sibling spelling identify the same directory on this volume?
	fn same_destination(&self, left: &Path, right: &Path) -> Result<bool, Problem>;

	/// Record the requested changes and bind their removal targets.
	fn begin_amend(
		&self,
		guard: &Self::Admission,
		task: &Task,
		request: &AmendRequest,
		targets: &[crate::RemovalTarget],
	) -> Result<Self::Checkpoint, Problem>;

	/// Read the participants bound to the pending removal.
	fn removal_targets(
		&self,
		guard: &Self::Admission,
	) -> Result<Vec<crate::RemovalTarget>, Problem>;

	/// Publish confirmed unlink and compacted membership as a single revision.
	fn settle_unlink(
		&self,
		guard: &Self::Admission,
		task: &Task,
		target: &crate::RemovalTarget,
	) -> Result<(), Problem>;

	/// Restore the checkpoint when the amendment produced no effects.
	fn unchanged_amend(
		&self,
		guard: &Self::Admission,
		checkpoint: Self::Checkpoint,
	) -> Result<(), Problem>;

	/// Read the recorded request for amendment recovery.
	fn amend_request(&self, guard: &Self::Admission) -> Result<Option<AmendRequest>, Problem>;
}
