/// Removal evidence belongs to one task incarnation. Restoration never toggles back.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RemovalDirection {
	Remove,
	Restore,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RemovalPhase {
	Present,
	Removed,
	Restoring,
	/// Supervised add succeeded; identity/final publication may still need recovery.
	Recreated,
	Restored,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RemovalParticipant {
	/// Local branch retained by removal; absent for a protected detached HEAD.
	pub branch: Option<String>,
	pub dirty_at_start: bool,
	pub phase: RemovalPhase,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Removal {
	pub direction: RemovalDirection,
	pub participants: Vec<RemovalParticipant>,
	/// Unix milliseconds, assigned only once every participant is removed.
	pub retired_at: Option<u64>,
}

impl Removal {
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
	pub nothing_to_recover: bool,
	pub task: crate::Task,
	pub record: std::path::PathBuf,
	pub operation: crate::Operation,
	pub problem: Option<crate::Problem>,
	/// True after entering publication, even if its durability is uncertain.
	pub mutation_started: bool,
	pub inspected: bool,
	pub record_available: bool,
	/// Existing damage/incomplete work, independent of this invocation outcome.
	pub needs_attention: bool,
}
