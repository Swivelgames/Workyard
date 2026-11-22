use crate::{Control, Git, Participant, Problem, Repository};
use std::{future::Future, path::PathBuf};

/// Retained branch or commit history disclosed during unlinking.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RetainedHistory {
	/// No unpushed commits were reported.
	UpToDate,
	/// Local commits have yet to reach the upstream.
	Unpushed,
	/// The local branch has no configured upstream.
	NoUpstream,
	/// The worktree is detached from a local branch.
	Detached,
}

/// Git facts used to decide whether unlinking needs consent.
#[derive(Clone, Debug)]
pub struct RemovalObservation {
	/// Identity of the worktree's Git administration entry.
	pub administration_identity: String,
	/// Native filesystem identity recorded for the worktree entry.
	pub worktree_entry: (u64, u64),
	/// Verified backing repository for this participant.
	pub repository: Repository,
	/// Location of the participant worktree.
	pub destination: Option<PathBuf>,
	/// Branch associated with the worktree, when known.
	pub branch: Option<String>,
	/// Local changes that could be discarded by unlinking.
	pub dirty_paths: Vec<PathBuf>,
	/// Content-sensitive token scoped to this inspection, never durable consent.
	pub dirty_token: Vec<u8>,
	/// Observed upstream relationship of the retained branch.
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
	/// The unlink command had not started.
	BeforeLaunch(Problem),
	/// The unlink command was attempted.
	Command(Problem),
}

impl From<Problem> for UnlinkFailure {
	fn from(problem: Problem) -> Self {
		Self::BeforeLaunch(problem)
	}
}

/// Inspect, unlink and restore participants while preserving retained history.
pub trait RemovalGit: Git {
	/// Only for durable successful recreation evidence, never a failed add attempt.
	fn reconcile_recreation(
		&self,
		participant: &Participant,
		branch: &str,
		control: &Control,
	) -> impl Future<Output = Result<crate::RecoveryObservation, Problem>> + Send;

	/// Verify a worktree recreated during removal recovery.
	fn reconcile_restored(
		&self,
		participant: &Participant,
		control: &Control,
	) -> impl Future<Output = Result<crate::RecoveryObservation, Problem>> + Send;

	/// Collect identity, history and local changes before unlinking.
	fn inspect_removal(
		&self,
		participant: &Participant,
		control: &Control,
	) -> impl Future<Output = Result<RemovalObservation, Problem>> + Send;

	/// Unlink the verified worktree under the selected removal policy.
	fn remove_worktree(
		&self,
		observation: &RemovalObservation,
		discard: bool,
		control: &Control,
		before_unlink: impl FnOnce() -> Result<(), Problem> + Send,
	) -> impl Future<Output = Result<(), UnlinkFailure>> + Send;

	/// Check whether the retained branch and destination permit restoration.
	fn inspect_restoration(
		&self,
		participant: &Participant,
		branch: &str,
		control: &Control,
	) -> impl Future<Output = Result<Repository, Problem>> + Send;
}

/// Kind of user consent requested before continuing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DecisionKind {
	/// Confirm retirement of the entire task.
	RemoveTask,
	/// Confirm removal of the selected members.
	RemoveParticipants,
	/// Authorize discarding changes in the selected participants.
	DiscardParticipants,
	/// Authorize discarding changes during task retirement.
	DiscardTask,
	/// Authorize newly disclosed local file loss.
	DiscardFiles,
	/// Retained branch or commit history disclosed during unlinking.
	RetainedHistory,
	/// Acknowledge that restoration cannot reproduce discarded edits.
	LostChanges,
}

/// Context supplied to a view when an operation needs consent.
#[derive(Clone, Debug)]
pub struct Decision {
	/// Task selected for this operation.
	pub task: Option<String>,
	/// Classification used to interpret this record.
	pub kind: DecisionKind,
	/// Participant to which this observation or change applies.
	pub participant: String,
	/// Members associated with this task or operation.
	pub participants: Vec<String>,
	/// Human-readable explanation for the caller.
	pub message: String,
}

/// Receives removal disclosures and requests for consent.
pub trait RemovalObserver: crate::Observer {
	/// Scope the disclosure to the task incarnation under mutation admission.
	fn begin_disclosure(&self, _task: &crate::Task, _amendment: Option<&crate::AmendRequest>) {}

	/// Exact approvals must also notice when previously disclosed changes have been saved.
	fn requires_unchanged_disclosure(&self) -> bool {
		false
	}

	/// Report removal facts before asking for a decision.
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

/// Consent and recovery direction for removal.
#[derive(Clone, Copy, Debug, Default)]
pub struct RemovalOptions {
	/// Accept routine confirmation.
	pub yes: bool,
	/// Authorize the disclosed discard and accept routine confirmation.
	pub force: bool,
	/// Resume in the restoration direction.
	pub undo: bool,
}
