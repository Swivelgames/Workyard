use super::super::RecoverOutcome;
use crate::{contracts::Progress, execution::removal::RemovalEvents};
use trine::InvocationContext;
use workyard_core::{Control, OperationOutcome};
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
	let observer = RemovalEvents::new(context, "recover", name.to_owned());
	let result =
		workyard_core::recover_amend(request, accepted, runner, control, &observer, store).await;

	let mut outcome = match result {
		Ok(result) => {
			let nothing = !result.mutation_started;
			let registration =
				crate::projects::register(store, &result.admitted_roots, control).await;
			let mut outcome = RecoverOutcome::from_task(result.task, result.record);
			outcome.registration_problem = registration.map(Into::into);
			outcome.pull_failures = result.pull_failures.into_iter().map(Into::into).collect();
			outcome.problem = result.problem.map(Into::into);
			outcome.operation = result.operation.into();
			outcome.changes = result.changes.into_iter().map(Into::into).collect();
			outcome.nothing_to_recover = nothing && outcome.problem.is_none();
			outcome
		}
		Err(problem) => rejected(problem),
	};
	outcome.removal = observer.into_report();

	if outcome.registration_problem.is_some()
		|| outcome.operation.outcome == OperationOutcome::Failed
	{
		Err(outcome)
	} else {
		Ok(outcome)
	}
}
