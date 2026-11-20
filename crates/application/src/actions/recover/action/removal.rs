use super::super::RecoverOutcome;
use crate::contracts::Progress;
use trine::InvocationContext;
use workyard_core::{Control, OperationOutcome, RemovalOptions};
use workyard_git::Runner;
use workyard_state::Store;

pub(super) async fn run(
	name: &str,
	options: RemovalOptions,
	runner: &Runner,
	control: &Control,
	context: InvocationContext<Progress>,
	store: &Store,
) -> Result<RecoverOutcome, RecoverOutcome> {
	let rejected = |problem| RecoverOutcome::rejected(name.to_owned(), problem);
	let observer =
		crate::execution::removal::RemovalEvents::new(context, "recover", name.to_owned());
	let result =
		workyard_core::recover_removal(name, options, runner, control, &observer, store).await;

	let mut outcome = match result {
		Ok(result) => RecoverOutcome::from_removal(result),
		Err(problem) => rejected(problem),
	};
	outcome.removal = observer.into_report();
	if outcome.operation.outcome == OperationOutcome::Failed {
		Err(outcome)
	} else {
		Ok(outcome)
	}
}
