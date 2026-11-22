use crate::Problem;
use std::path::PathBuf;

/// Progress of participant creation, distinct from task lifecycle.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Phase {
	/// No participant work has begun.
	NotStarted,
	/// Git is refreshing the selected repository's refs.
	Fetching,
	/// Worktree creation has begun.
	Creating,
	/// Git has created the participant worktree.
	WorktreeCreated,
	/// Setup for this participant has settled.
	Completed,
}

impl Phase {
	/// Return the stable spelling used in records or output.
	pub fn as_str(&self) -> &'static str {
		match self {
			Self::NotStarted => "not_started",
			Self::Fetching => "fetching",
			Self::Creating => "creating",
			Self::WorktreeCreated => "worktree_created",
			Self::Completed => "completed",
		}
	}
}

/// Last known relationship paths used as starting points for verification.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PlacementHint {
	/// Path to the backing repository.
	pub source: PathBuf,
	/// Git administration directory shared by the repository's worktrees.
	pub common_dir: PathBuf,
	/// Location of the participant worktree.
	pub destination: PathBuf,
	/// Workspace root containing the Workyard marker.
	pub workspace: PathBuf,
	/// Native filesystem identity recorded for the worktree entry.
	pub worktree_entry: Option<(u64, u64)>,
}

/// A repository's membership, branch choices and linked worktree in a task.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Participant {
	/// Previous verified backing and worktree paths, used to check manually moved links.
	pub placement: Option<PlacementHint>,
	/// Project discovery roots associated with this participant.
	pub registration_roots: Vec<PathBuf>,
	/// Branch and fetch policy recorded for this participant.
	pub creation: Option<super::creation::CreationOptions>,
	/// Directory name of this participant inside the task.
	pub name: String,
	/// Path to the backing repository.
	pub source: PathBuf,
	/// Git administration directory shared by the repository's worktrees.
	pub common_dir: PathBuf,
	/// Identity used to distinguish backing repositories across path changes.
	pub backing_identity: String,
	/// Identity used to verify the linked worktree before applying changes.
	pub worktree_identity: Option<String>,
	/// Location of the participant worktree.
	pub destination: PathBuf,
	/// Remote selected for branch resolution or fetch.
	pub remote: Option<String>,
	/// Starting revision for a newly created branch.
	pub from: Option<String>,
	/// Step reached by this participant.
	pub phase: Phase,
}

/// Whether a task is active or has been retired.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TaskLifecycle {
	/// The task is managed as current work.
	Active,
	/// The task has been closed and retained in history.
	Retired,
}

impl TaskLifecycle {
	/// Return the stable spelling used in records or output.
	pub fn as_str(self) -> &'static str {
		match self {
			Self::Active => "active",
			Self::Retired => "retired",
		}
	}
}

/// Membership, placement and operation history for one task incarnation.
#[derive(Clone, Debug)]
pub struct Task {
	/// Identifier distinguishing successive uses of the same task name.
	pub generation: u64,
	/// Removal or restoration evidence associated with this task.
	pub removal: Option<super::removal::Removal>,
	/// Branch name that identifies this task.
	pub name: String,
	/// Directory under which the task branch path is placed.
	pub root: PathBuf,
	/// Directory holding authoritative Workyard state.
	pub state_dir: PathBuf,
	/// Members associated with this task or operation.
	pub participants: Vec<Participant>,
	/// Allow reuse of an existing target branch.
	pub checkout: bool,
	/// Use locally available refs without fetching.
	pub no_fetch: bool,
	/// Current membership lifecycle of the task.
	pub lifecycle: TaskLifecycle,
	/// Invocation history tracked separately from task lifecycle.
	pub operation: super::operation::Operation,
	/// Diagnostic describing a failure or an unresolved condition.
	pub problem: Option<Problem>,
}

impl Task {
	/// A cancelled delivery can arrive after every participant was established.
	pub(crate) fn preserve_settled_cancellation(&mut self) {
		if self.operation.outcome == crate::OperationOutcome::Cancelled
			&& self
				.problem
				.as_ref()
				.is_some_and(|problem| problem.code == crate::ProblemCode::Cancelled)
			&& self
				.participants
				.iter()
				.all(|participant| participant.phase == Phase::Completed)
		{
			self.operation.outcome = crate::OperationOutcome::Succeeded;
			self.problem = None;
		}
	}

	/// Only a newly established worktree can publish its original discovery roots.
	/// A later no-op recovery must respect an intervening explicit untrack.
	pub fn admitted_roots(&self, previous: Option<&Self>) -> Vec<PathBuf> {
		let established: std::collections::HashSet<_> = previous
			.into_iter()
			.flat_map(|task| &task.participants)
			.filter(|p| p.worktree_identity.is_some())
			.map(|p| &p.backing_identity)
			.collect();
		self.participants
			.iter()
			.filter(|p| p.worktree_identity.is_some() && !established.contains(&p.backing_identity))
			.flat_map(|p| p.registration_roots.iter().cloned())
			.collect()
	}

	/// Use the participant’s creation policy, falling back to the task defaults.
	pub fn creation_options(&self, index: usize) -> super::creation::CreationOptions {
		self.participants[index]
			.creation
			.unwrap_or(super::creation::CreationOptions {
				checkout: self.checkout,
				no_fetch: self.no_fetch,
			})
	}
}
