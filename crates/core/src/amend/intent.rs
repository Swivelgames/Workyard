//! Select the amendment and any removal authority while task admission is held.
use crate::{
	AmendRequest, AmendState, OperationKind, OperationOutcome, Problem, ProblemCode,
	RecoveryRecord, RemovalOptions, TaskLifecycle,
};

/// Inputs needed to resume an interrupted amendment.
pub enum AmendRecovery {
	/// Continue adding or renaming participants.
	AddOrRename,
	/// Continue an amendment with explicit removal consent.
	WithRemovalOptions(RemovalOptions),
}

pub(super) enum Intent {
	Apply(AmendRequest),
	Resume(AmendRecovery),
}

impl Intent {
	pub(super) fn resolve<S: AmendState>(
		self,
		record: &RecoveryRecord,
		store: &S,
		guard: &S::Admission,
	) -> Result<AmendRequest, Problem> {
		match self {
			Self::Apply(request) => {
				if request.remove.is_empty()
					&& unfinished_amendment(record)
					&& store
						.amend_request(guard)?
						.is_some_and(|prior| !prior.remove.is_empty())
				{
					return Err(removal_decision_required());
				}
				Ok(request)
			}
			Self::Resume(scope) => {
				let mut request = store.amend_request(guard)?.ok_or_else(|| {
					Problem::new(
						ProblemCode::InvalidInput,
						"This task has no amendment to resume.",
					)
				})?;
				request.removal_options = scope.authorize(record, &request)?;
				Ok(request)
			}
		}
	}
}

impl AmendRecovery {
	fn authorize(
		&self,
		record: &RecoveryRecord,
		request: &AmendRequest,
	) -> Result<RemovalOptions, Problem> {
		if record.task.lifecycle != TaskLifecycle::Active
			|| record.task.removal.is_some()
			|| !record.task.operation.is_continuing(OperationKind::Amend)
		{
			return Err(Problem::new(
				ProblemCode::InvalidInput,
				"This task has no active participant amendment to resume.",
			));
		}
		if let Self::WithRemovalOptions(removal) = self {
			if (removal.yes || removal.force) && request.remove.is_empty() {
				return Err(Problem::new(
					ProblemCode::InvalidInput,
					"--yes and --force require a pending removal.",
				));
			}
			return Ok(*removal);
		}

		if !request.remove.is_empty() {
			return Err(removal_decision_required());
		}
		Ok(RemovalOptions::default())
	}
}

fn unfinished_amendment(record: &RecoveryRecord) -> bool {
	record.task.operation.is_continuing(OperationKind::Amend)
		&& (record.task.operation.outcome != OperationOutcome::Succeeded
			|| record.pending.is_some()
			|| record.needs_publication)
}

fn removal_decision_required() -> Problem {
	Problem::new(
		ProblemCode::InvalidInput,
		"The recorded amendment includes participant removal.",
	)
	.remedy(
		"Inspect the task with status. Repeat the original amendment to continue its selected removals, or use CLI or RPC recovery.",
	)
}
