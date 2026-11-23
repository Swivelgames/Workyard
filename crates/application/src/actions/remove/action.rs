use super::{RemoveInput, RemoveOutcome};
use crate::{
	contracts::Progress,
	execution::{
		blocking_action,
		removal::{RemovalConsent, RemovalEvents, RemovalRequest},
	},
};
use trine::InvocationContext;
use workyard_core::{Control, OperationOutcome, ProblemCode, RemovalOptions};

/// Retire the task using the supplied confirmation and discard policy.
pub async fn remove(
	input: RemoveInput,
	context: InvocationContext<Progress>,
) -> Result<RemoveOutcome, RemoveOutcome> {
	execute(input.into(), context).await
}

pub(super) async fn execute(
	request: RemovalRequest<RemoveInput>,
	context: InvocationContext<Progress>,
) -> Result<RemoveOutcome, RemoveOutcome> {
	blocking_action(run(request, context)).await
}

async fn run(
	request: RemovalRequest<RemoveInput>,
	context: InvocationContext<Progress>,
) -> Result<RemoveOutcome, RemoveOutcome> {
	let input = request.input;
	let rejected = |problem| {
		let mut outcome = RemoveOutcome::rejected(input.task.clone(), problem);
		outcome.explicit_workspace = input.workspace.clone();
		outcome
	};
	workyard_core::validate_task(&input.task).map_err(rejected)?;

	let cancellation = context.cancellation().clone();
	let control = Control::new(move || cancellation.is_cancelled());
	control.check().map_err(rejected)?;

	let state =
		crate::workspace::required(input.workspace.as_ref().map(trine::NativePath::as_path))
			.map_err(rejected)?;
	let store = workyard_state::Store::open_existing(&state, None).map_err(rejected)?;
	let runner = workyard_git::Runner::default();
	let forward_only = matches!(request.consent, RemovalConsent::ReviewDiscard { .. });
	let observer = RemovalEvents::new(context, "remove", input.task.clone())
		.with_consent(request.consent, &state);

	let result = if forward_only {
		workyard_core::retire(&input.task, &runner, &control, &observer, &store).await
	} else {
		let options = RemovalOptions {
			yes: input.yes,
			force: input.force,
			undo: false,
		};

		workyard_core::remove(&input.task, options, &runner, &control, &observer, &store).await
	};

	let mut outcome = match result {
		Ok(result) => RemoveOutcome::from_removal(result),
		Err(problem) => rejected(problem),
	};
	outcome.removal = observer.into_report();
	outcome.explicit_workspace = input.workspace;
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
