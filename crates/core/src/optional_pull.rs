//! Optional Git update settlement and invocation reporting policy.
use crate::{EffectState, Phase, Problem, State, Task};

/// Settled pull outcomes do not revoke successfully created membership.
/// Unsupervised writers retain their pending evidence; cancellation still stops work.
pub(crate) fn settle_pull<S: State>(
	result: Result<(), Problem>,
	task: &mut Task,
	index: usize,
	store: &S,
	guard: &S::Admission,
) -> Result<Option<crate::PullFailure>, Problem> {
	if result
		.as_ref()
		.is_err_and(|problem| problem.code == crate::ProblemCode::SupervisionFailed)
	{
		return result.map(|_| None);
	}

	task.participants[index].phase = Phase::Completed;
	store.record_effect(guard, task, index, EffectState::Settled)?;
	match result {
		Ok(()) => Ok(None),
		Err(problem) if problem.code == crate::ProblemCode::Cancelled => Err(problem),
		Err(mut problem) => {
			problem.remedy = "The worktree is established. Inspect git status there and resolve or retry the pull with Git; Workyard recovery or acceptance is not required.".into();
			Ok(Some(crate::PullFailure {
				participant: task.participants[index].name.clone(),
				problem,
			}))
		}
	}
}

pub(crate) fn invocation_operation(
	mut operation: crate::Operation,
	failures: &[crate::PullFailure],
) -> crate::Operation {
	if !failures.is_empty() && operation.outcome == crate::OperationOutcome::Succeeded {
		operation.outcome = crate::OperationOutcome::Failed;
	}
	operation
}

/// A cancelled optional update can leave every member settled. Keep its invocation
/// cancellation separate; only actual incomplete membership requires continuation.
pub(crate) fn preserve_settled_cancellation(task: &mut Task) {
	if task.operation.outcome == crate::OperationOutcome::Cancelled
		&& task
			.problem
			.as_ref()
			.is_some_and(|problem| problem.code == crate::ProblemCode::Cancelled)
		&& task
			.participants
			.iter()
			.all(|participant| participant.phase == Phase::Completed)
	{
		task.operation.outcome = crate::OperationOutcome::Succeeded;
		task.problem = None;
	}
}
