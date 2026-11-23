use super::{StatusInput, StatusOutcome};
use crate::{contracts::Progress, execution::blocking_action};
use trine::InvocationContext;

/// Inspect live Git state for the selected task or active workspace tasks.
pub async fn status(
	input: StatusInput,
	context: InvocationContext<Progress>,
) -> Result<StatusOutcome, StatusOutcome> {
	blocking_action(run(input, context)).await
}

async fn run(
	input: StatusInput,
	context: InvocationContext<Progress>,
) -> Result<StatusOutcome, StatusOutcome> {
	crate::observation::read(
		input.task,
		input.workspace,
		crate::observation::Selection::Active,
		context,
	)
	.await
	.map(StatusOutcome)
	.map_err(StatusOutcome)
}
