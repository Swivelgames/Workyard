use crate::{OperationKind, Problem, ProblemCode, Task, TaskLifecycle};

/// Authority granted to a recovery invocation.
pub enum RecoveryScope {
	/// Include older active tasks whose records have no operation provenance.
	Reconcile,
	/// Continue only an active create or checkout, including a previous continuation.
	CreateOnly,
}

impl RecoveryScope {
	pub(super) fn authorize(self, task: &Task) -> Result<(), Problem> {
		let owner = task.operation.continues.or(task.operation.kind);
		let permitted = match owner {
			Some(OperationKind::Create) => true,
			None | Some(OperationKind::Recover) => matches!(self, Self::Reconcile),
			Some(OperationKind::Amend | OperationKind::Remove) => false,
		};
		if task.lifecycle == TaskLifecycle::Active && task.removal.is_none() && permitted {
			return Ok(());
		}

		let operation = owner.map_or("unknown", OperationKind::as_str);
		let message = format!(
			"Creation recovery requires an active create or checkout. Task {:?} is {} with recorded operation {operation}.",
			task.name,
			task.lifecycle.as_str()
		);
		Err(Problem::new(ProblemCode::InvalidInput, message).remedy(
			"Inspect this task with status. Repeat the original removal or amendment to continue it. Use CLI or RPC recovery for restoration.",
		))
	}
}
