use crate::{Control, Git, Participant, Problem, Repository};
use std::{future::Future, path::PathBuf};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RetainedHistory {
	UpToDate,
	Unpushed,
	NoUpstream,
	Detached,
}

#[derive(Clone, Debug)]
pub struct RemovalObservation {
	pub administration_identity: String,
	pub worktree_entry: (u64, u64),
	pub repository: Repository,
	pub destination: Option<PathBuf>,
	pub branch: Option<String>,
	pub dirty_paths: Vec<PathBuf>,
	/// Content-sensitive token scoped to this inspection, never durable consent.
	pub dirty_token: Vec<u8>,
	pub history: RetainedHistory,
}

impl RemovalObservation {
	pub(crate) fn same_worktree(&self, other: &Self) -> bool {
		self.destination == other.destination
			&& self.administration_identity == other.administration_identity
			&& self.worktree_entry == other.worktree_entry
	}
}

/// Only a destructive command can leave behind an unconfirmed unlink writer.
#[derive(Debug)]
pub enum UnlinkFailure {
	BeforeLaunch(Problem),
	Command(Problem),
}

impl From<Problem> for UnlinkFailure {
	fn from(problem: Problem) -> Self {
		Self::BeforeLaunch(problem)
	}
}

pub trait RemovalGit: Git {
	/// Only for durable successful recreation evidence, never a failed add attempt.
	fn reconcile_recreation(
		&self,
		participant: &Participant,
		branch: &str,
		control: &Control,
	) -> impl Future<Output = Result<crate::RecoveryObservation, Problem>> + Send;

	fn reconcile_restored(
		&self,
		participant: &Participant,
		control: &Control,
	) -> impl Future<Output = Result<crate::RecoveryObservation, Problem>> + Send;

	fn inspect_removal(
		&self,
		participant: &Participant,
		control: &Control,
	) -> impl Future<Output = Result<RemovalObservation, Problem>> + Send;
	fn remove_worktree(
		&self,
		observation: &RemovalObservation,
		discard: bool,
		control: &Control,
		before_unlink: impl FnOnce() -> Result<(), Problem> + Send,
	) -> impl Future<Output = Result<(), UnlinkFailure>> + Send;
	fn inspect_restoration(
		&self,
		participant: &Participant,
		branch: &str,
		control: &Control,
	) -> impl Future<Output = Result<Repository, Problem>> + Send;
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DecisionKind {
	RemoveTask,
	RemoveParticipants,
	DiscardParticipants,
	DiscardTask,
	DiscardFiles,
	RetainedHistory,
	LostChanges,
}

#[derive(Clone, Debug)]
pub struct Decision {
	pub task: Option<String>,
	pub kind: DecisionKind,
	pub participant: String,
	pub participants: Vec<String>,
	pub message: String,
}

pub trait RemovalObserver: crate::Observer {
	/// Scope the disclosure to the task incarnation under mutation admission.
	fn begin_disclosure(&self, _task: &crate::Task, _amendment: Option<&crate::AmendRequest>) {}

	/// Exact approvals must also notice when previously disclosed changes have been saved.
	fn requires_unchanged_disclosure(&self) -> bool {
		false
	}

	fn disclose(
		&self,
		_participant: &str,
		_observation: &RemovalObservation,
	) -> impl Future<Output = Result<(), Problem>> + Send {
		async { Ok(()) }
	}
	/// Forget a completed target before any later discard decision.
	fn removed(
		&self,
		task: &str,
		participant: &str,
	) -> impl Future<Output = Result<(), Problem>> + Send {
		self.phase(task, participant, "removed")
	}

	/// Returns None when the interface cannot ask; never silently grants consent.
	fn confirm(
		&self,
		_decision: Decision,
	) -> impl Future<Output = Result<Option<bool>, Problem>> + Send {
		async { Ok(None) }
	}
}

#[derive(Clone, Copy, Debug, Default)]
pub struct RemovalOptions {
	pub yes: bool,
	pub force: bool,
	pub undo: bool,
}
