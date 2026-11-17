use super::super::RecoverOutcome;
use crate::{contracts::Progress, execution::Events};
use trine::InvocationContext;
use workyard_core::{Control, OperationOutcome, ProblemCode};
use workyard_git::Runner;
use workyard_state::Store;

pub(super) async fn run(
	request: workyard_core::AmendRequest,
	name: &str,
	accepted: &[String],
	runner: &Runner,
	control: &Control,
	context: InvocationContext<Progress>,
	store: &Store,
) -> Result<RecoverOutcome, RecoverOutcome> {
	let rejected = |problem| RecoverOutcome::rejected(name.to_owned(), problem);
	let result = workyard_core::recover_amend(
		request,
		accepted,
		runner,
		control,
		&Events(context, "recover"),
		store,
	)
	.await;

	match result {
		Ok(result) => {
			let nothing = !result.mutation_started;
			let mut outcome = RecoverOutcome::from_task(result.task, result.record);
			outcome.pull_failures = result.pull_failures.into_iter().map(Into::into).collect();
			outcome.problem = result.problem.map(Into::into);
			outcome.operation = result.operation.into();
			outcome.nothing_to_recover = nothing && outcome.problem.is_none();
			if outcome.operation.outcome == OperationOutcome::Failed {
				Err(outcome)
			} else {
				Ok(outcome)
			}
		}
		Err(problem) if problem.code == ProblemCode::Cancelled => Ok(rejected(problem)),
		Err(problem) => Err(rejected(problem)),
	}
}
