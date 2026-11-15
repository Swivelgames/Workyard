use super::{StatusInput, StatusOutcome};
use crate::{common::Progress, invocation::blocking_action};
use trine::InvocationContext;

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
		input.tasks_root,
		input.state_dir,
		crate::observation::Selection::Active,
		context,
	)
	.await
	.map(StatusOutcome)
	.map_err(StatusOutcome)
}
