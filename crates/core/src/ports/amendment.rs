use crate::{
	AmendRequest, Control, Participant, Problem, RecoveryGit, RecoveryState, Repository, Task,
};
use std::{future::Future, path::Path};

pub struct Relationship {
	pub participant: Participant,
	pub needs_repair: bool,
}

pub trait AmendGit: RecoveryGit + crate::RemovalGit + crate::ObservationGit {
	fn relationship(
		&self,
		repository: &Repository,
		participant: &Participant,
		control: &Control,
	) -> impl Future<Output = Result<Relationship, Problem>> + Send;
	fn repair(
		&self,
		repository: &Repository,
		participant: &Participant,
		control: &Control,
	) -> impl Future<Output = Result<(), Problem>> + Send;
	fn move_worktree(
		&self,
		repository: &Repository,
		participant: &Participant,
		destination: &Path,
		control: &Control,
	) -> impl Future<Output = Result<(), Problem>> + Send;
}

pub trait AmendState: RecoveryState {
	type Checkpoint: Send;
	/// May a private staging name be used without adopting an existing entry?
	fn staging_available(&self, path: &Path) -> Result<bool, Problem>;
	/// Does this sibling spelling identify the same directory on this volume?
	fn same_destination(&self, left: &Path, right: &Path) -> Result<bool, Problem>;
	fn begin_amend(
		&self,
		guard: &Self::Admission,
		task: &Task,
		request: &AmendRequest,
		targets: &[crate::RemovalTarget],
	) -> Result<Self::Checkpoint, Problem>;
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
	fn unchanged_amend(
		&self,
		guard: &Self::Admission,
		checkpoint: Self::Checkpoint,
	) -> Result<(), Problem>;
	fn amend_request(&self, guard: &Self::Admission) -> Result<Option<AmendRequest>, Problem>;
}
