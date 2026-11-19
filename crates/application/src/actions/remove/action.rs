use super::{RemoveInput, RemoveOutcome};
use crate::{contracts::Progress, execution::blocking_action, execution::removal::RemovalEvents};
use trine::InvocationContext;
use workyard_core::{Control, OperationOutcome, ProblemCode, RemovalOptions};

pub async fn remove(
	input: RemoveInput,
	context: InvocationContext<Progress>,
) -> Result<RemoveOutcome, RemoveOutcome> {
	blocking_action(run(input, context)).await
}

async fn run(
	input: RemoveInput,
	context: InvocationContext<Progress>,
) -> Result<RemoveOutcome, RemoveOutcome> {
	let rejected = |problem| RemoveOutcome::rejected(input.task.clone(), problem);
	workyard_core::validate_task(&input.task).map_err(rejected)?;

	let cancellation = context.cancellation().clone();
	let control = Control::new(move || cancellation.is_cancelled());
	control.check().map_err(rejected)?;

	let state =
		crate::workspace::required(input.workspace.as_ref().map(trine::NativePath::as_path))
			.map_err(rejected)?;
	let store = workyard_state::Store::open_existing(&state, None).map_err(rejected)?;
	let runner = workyard_git::Runner::default();
	let observer = RemovalEvents::new(context, "remove", input.task.clone());
	let options = RemovalOptions {
		yes: input.yes,
		force: input.force,
		undo: false,
	};

	let result =
		workyard_core::remove(&input.task, options, &runner, &control, &observer, &store).await;

	let mut outcome = match result {
		Ok(result) => RemoveOutcome::from_removal(result),
		Err(problem) => rejected(problem),
	};
	outcome.disclosures = observer.disclosures.into_inner().expect("disclosures");
	outcome.decision = observer.decision.into_inner().expect("decision");
	if outcome.operation.outcome == OperationOutcome::Failed
		&& outcome
			.problem
			.as_ref()
			.is_none_or(|problem| problem.code != ProblemCode::Cancelled.as_str())
	{
		Err(outcome)
	} else {
		Ok(outcome)
	}
}
