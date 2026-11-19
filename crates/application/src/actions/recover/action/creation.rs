use super::super::RecoverOutcome;
use crate::{contracts::Progress, execution::Events};
use trine::InvocationContext;
use workyard_core::{Control, OperationOutcome, ProblemCode};
use workyard_git::Runner;
use workyard_state::Store;

pub(super) async fn run(
	name: &str,
	accepted: &[String],
	runner: &Runner,
	control: &Control,
	context: InvocationContext<Progress>,
	store: &Store,
) -> Result<RecoverOutcome, RecoverOutcome> {
	let rejected = |problem| RecoverOutcome::rejected(name.to_owned(), problem);
	let result = workyard_core::recover(
		name,
		accepted,
		runner,
		control,
		&Events(context, "recover"),
		store,
	)
	.await;

	match result {
		Ok(result) => {
			let registration =
				crate::projects::register(store, &result.admitted_roots, control).await;
			let mut outcome = RecoverOutcome::from_task(result.task, result.record);
			outcome.registration_problem = registration.map(Into::into);
			outcome.pull_failures = result.pull_failures.into_iter().map(Into::into).collect();
			outcome.problem = result.problem.map(Into::into);
			outcome.operation = result.operation.into();
			outcome.nothing_to_recover = result.nothing_to_recover;
			if outcome.nothing_to_recover {
				outcome.operation.continues = outcome
					.operation
					.continues
					.or(outcome.operation.kind.filter(|kind| *kind != "recover"));
				outcome.operation.kind = Some("recover");
			}
			if outcome.registration_problem.is_some()
				|| outcome.operation.outcome == OperationOutcome::Failed
			{
				Err(outcome)
			} else {
				Ok(outcome)
			}
		}
		Err(problem) if problem.code == ProblemCode::Cancelled => Ok(rejected(problem)),
		Err(problem) => Err(rejected(problem)),
	}
}
