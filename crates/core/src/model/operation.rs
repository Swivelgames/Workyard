/// The operation requested by the caller, distinct from a task's lifecycle.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OperationKind {
	Create,
	Amend,
	Recover,
	Remove,
}

impl OperationKind {
	pub fn as_str(self) -> &'static str {
		match self {
			Self::Amend => "amend",
			Self::Create => "create",
			Self::Recover => "recover",
			Self::Remove => "remove",
		}
	}
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OperationOutcome {
	Pending,
	Succeeded,
	Cancelled,
	Failed,
}

impl OperationOutcome {
	pub fn as_str(self) -> &'static str {
		match self {
			Self::Pending => "pending",
			Self::Succeeded => "succeeded",
			Self::Cancelled => "cancelled",
			Self::Failed => "failed",
		}
	}
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Operation {
	/// Unknown for records written before operation provenance was recorded.
	pub kind: Option<OperationKind>,
	/// Recovery continues an existing operation; it does not replace its meaning.
	pub continues: Option<OperationKind>,
	pub outcome: OperationOutcome,
}

impl Operation {
	pub fn create() -> Self {
		Self {
			kind: Some(OperationKind::Create),
			continues: None,
			outcome: OperationOutcome::Pending,
		}
	}

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
