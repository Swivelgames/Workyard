use super::super::RecoverOutcome;
use crate::contracts::Progress;
use trine::InvocationContext;
use workyard_core::{Control, OperationOutcome, ProblemCode};
use workyard_git::Runner;
use workyard_state::Store;

use super::super::RecoverInput;

pub(super) async fn run(
	input: RecoverInput,
	context: InvocationContext<Progress>,
	runner: &Runner,
	control: &Control,
	store: &Store,
) -> Result<RecoverOutcome, RecoverOutcome> {
	let name = input.task.clone();
	let rejected = |problem| RecoverOutcome::rejected(name.clone(), problem);
	if !input.accept_pull.is_empty() {
		return Err(rejected(workyard_core::Problem::new(
			ProblemCode::InvalidInput,
			"--accept-pull does not apply to removal.",
		)));
	}
	let observer = crate::execution::removal::RemovalEvents::new(context, "recover", name.clone());
	let options = workyard_core::RemovalOptions {
		undo: input.undo,
		yes: input.yes,
		force: input.force,
	};
	let result =
		workyard_core::recover_removal(&name, options, runner, control, &observer, store).await;

	let mut outcome = match result {
		Ok(result) => RecoverOutcome::from_removal(result),
		Err(problem) => rejected(problem),
	};
	outcome.disclosures = observer.disclosures.into_inner().expect("disclosures");
	outcome.decision = observer.decision.into_inner().expect("decision");
	if outcome.operation.outcome == OperationOutcome::Failed {
		Err(outcome)
	} else {
		Ok(outcome)
	}
}
