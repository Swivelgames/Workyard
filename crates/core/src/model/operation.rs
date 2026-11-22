/// The operation requested by the caller, distinct from a task's lifecycle.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OperationKind {
	/// Establish a task and its linked worktrees.
	Create,
	/// Change an existing task's membership.
	Amend,
	/// Continue or reconcile recorded work.
	Recover,
	/// Unlink participant worktrees.
	Remove,
}

impl OperationKind {
	/// Return the stable spelling used in records or output.
	pub fn as_str(self) -> &'static str {
		match self {
			Self::Amend => "amend",
			Self::Create => "create",
			Self::Recover => "recover",
			Self::Remove => "remove",
		}
	}
}

/// Settlement of one operation, independent of task health.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OperationOutcome {
	/// Work has started and final settlement is outstanding.
	Pending,
	/// The operation finished successfully.
	Succeeded,
	/// The caller interrupted the operation.
	Cancelled,
	/// The operation encountered a problem.
	Failed,
}

impl OperationOutcome {
	/// Return the stable spelling used in records or output.
	pub fn as_str(self) -> &'static str {
		match self {
			Self::Pending => "pending",
			Self::Succeeded => "succeeded",
			Self::Cancelled => "cancelled",
			Self::Failed => "failed",
		}
	}
}

/// What Workyard attempted and how that attempt settled.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Operation {
	/// Unknown for records written before operation provenance was recorded.
	pub kind: Option<OperationKind>,
	/// Recovery continues an existing operation; it does not replace its meaning.
	pub continues: Option<OperationKind>,
	/// Settlement of this invocation.
	pub outcome: OperationOutcome,
}

impl Operation {
	/// Start recording a pending create operation.
	pub fn create() -> Self {
		Self {
			kind: Some(OperationKind::Create),
			continues: None,
			outcome: OperationOutcome::Pending,
		}
	}

	/// A direct operation or a recovery explicitly continuing that operation.
	pub fn is_continuing(self, kind: OperationKind) -> bool {
		match (self.kind, self.continues) {
			(Some(OperationKind::Recover), Some(previous)) => previous == kind,
			(Some(current), None) => current == kind,
			_ => false,
		}
	}

	/// Start a recovery operation carrying forward the original operation kind.
	pub fn recovery(self) -> Self {
		Self {
			kind: Some(OperationKind::Recover),
			continues: self
				.continues
				.or(self.kind.filter(|kind| *kind != OperationKind::Recover)),
			outcome: OperationOutcome::Pending,
		}
	}
}
