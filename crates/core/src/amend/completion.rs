use super::Amendment;
use crate::{
	AmendGit, AmendRequest, AmendResult, AmendState, Operation, OperationKind, OperationOutcome,
	Problem, ProblemCode, RecoveryRecord, RemovalObserver, Task,
};

pub(super) fn needs_publication(record: &RecoveryRecord, request: &AmendRequest) -> bool {
	if record.recorded.is_none() || record.needs_publication {
		return true;
	}
	// Recovery still checks every requested change. A cancelled add/rename can
	// leave an intact record even when its last progress message wasn't delivered.
	match record.task.operation.outcome {
		OperationOutcome::Succeeded => false,
		OperationOutcome::Cancelled => record.task.problem.is_some() || !request.remove.is_empty(),
		OperationOutcome::Pending | OperationOutcome::Failed => true,
	}
}

impl<G: AmendGit, S: AmendState, O: RemovalObserver> Amendment<'_, G, S, O> {
	pub(super) fn finish(
		mut self,
		mut task: Task,
		original: Task,
		result: Result<(), Problem>,
		recovering: bool,
	) -> Result<AmendResult, Problem> {
		let operation = Operation {
			kind: Some(if recovering {
				OperationKind::Recover
			} else {
				OperationKind::Amend
			}),
			continues: recovering.then_some(OperationKind::Amend),
			outcome: match &result {
				Ok(()) => OperationOutcome::Succeeded,
				Err(problem) if problem.code == ProblemCode::Cancelled => {
					OperationOutcome::Cancelled
				}
				Err(_) => OperationOutcome::Failed,
			},
		};
		let mut problem = result.err();
		let admitted_roots = task.admitted_roots(Some(&original));
		if !self.changed && !self.uncertain {
			if let Some(checkpoint) = self.checkpoint.take() {
				self.store.unchanged_amend(self.guard, checkpoint)?;
			}
			task = original;
		} else {
			// Invocation errors need not become task damage. Incomplete participant facts
			// and pending evidence already describe genuine unfinished work.
			task.operation = operation;
			if task
				.participants
				.iter()
				.all(|p| p.phase == crate::Phase::Completed)
				&& task
					.problem
					.as_ref()
					.is_some_and(|_| original.operation.kind == Some(OperationKind::Amend))
			{
				task.problem = None;
			}
			if !self.uncertain
				&& let Err(error) = self.store.save(self.guard, &task)
			{
				problem = Some(error);
			}
		}
		let operation = if problem.is_some() && operation.outcome == OperationOutcome::Succeeded {
			Operation {
				outcome: OperationOutcome::Failed,
				..operation
			}
		} else {
			operation
		};
		Ok(AmendResult {
			admitted_roots,
			mutation_started: self.changed || self.uncertain,
			task,
			record: self.store.record_path(self.guard),
			operation,
			changes: self.changes,
			problem,
		})
	}
}
