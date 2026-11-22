/// Removal evidence belongs to one task incarnation. Restoration never toggles back.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RemovalDirection {
	/// Unlink participant worktrees.
	Remove,
	/// Recreate previously unlinked worktrees from retained branches.
	Restore,
}

/// Progress of a participant through unlinking or restoration.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RemovalPhase {
	/// The participant still has its original worktree.
	Present,
	/// The participant worktree has been unlinked.
	Removed,
	/// Restoration has begun.
	Restoring,
	/// Supervised add succeeded; identity/final publication may still need recovery.
	Recreated,
	/// Restoration of the worktree has settled.
	Restored,
}

/// Facts retained for one participant across removal and undo.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RemovalParticipant {
	/// Local branch retained by removal; absent for a protected detached HEAD.
	pub branch: Option<String>,
	/// Whether local changes were present when removal began.
	pub dirty_at_start: bool,
	/// Step reached by this participant.
	pub phase: RemovalPhase,
}

/// Evidence retained to continue or undo removal.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Removal {
	/// Direction persisted for subsequent recovery attempts.
	pub direction: RemovalDirection,
	/// Members associated with this task or operation.
	pub participants: Vec<RemovalParticipant>,
	/// Unix milliseconds, assigned only once every participant is removed.
	pub retired_at: Option<u64>,
}

impl Removal {
	/// Check whether every participant is present or restored in the restoration direction.
	pub fn restoration_complete(&self) -> bool {
		self.direction == RemovalDirection::Restore
			&& self
				.participants
				.iter()
				.all(|p| matches!(p.phase, RemovalPhase::Present | RemovalPhase::Restored))
	}
}

/// An invocation outcome does not replace task history when nothing was changed.
pub struct RemovalResult {
	/// Whether inspection found no outstanding work to resume.
	pub nothing_to_recover: bool,
	/// Task selected for this operation.
	pub task: crate::Task,
	/// Location of the task record when available.
	pub record: std::path::PathBuf,
	/// Invocation history tracked separately from task lifecycle.
	pub operation: crate::Operation,
	/// Diagnostic describing a failure or an unresolved condition.
	pub problem: Option<crate::Problem>,
	/// True after entering publication, even if its durability is uncertain.
	pub mutation_started: bool,
	/// Whether inspection produced removal facts.
	pub inspected: bool,
	/// Whether a task record was available for the result.
	pub record_available: bool,
	/// Existing damage/incomplete work, independent of this invocation outcome.
	pub needs_attention: bool,
}
