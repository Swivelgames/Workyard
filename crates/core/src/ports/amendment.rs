use crate::{
	AmendRequest, Control, Participant, Problem, RecoveryGit, RecoveryState, Repository, Task,
};
use std::{future::Future, path::Path};

pub struct Relationship {
	pub participant: Participant,
	pub needs_repair: bool,
}

pub trait AmendGit: RecoveryGit {
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
	type Checkpoint;
	fn begin_amend(
		&self,
		guard: &Self::Admission,
		task: &Task,
		request: &AmendRequest,
	) -> Result<Self::Checkpoint, Problem>;
	fn unchanged_amend(
		&self,
		guard: &Self::Admission,
		checkpoint: Self::Checkpoint,
	) -> Result<(), Problem>;
	fn amend_request(&self, guard: &Self::Admission) -> Result<Option<AmendRequest>, Problem>;
}
