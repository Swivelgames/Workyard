use super::{ListInput, ListOutcome};
use crate::{common::Progress, invocation::blocking_action};
use trine::InvocationContext;

pub async fn list(
	input: ListInput,
	context: InvocationContext<Progress>,
) -> Result<ListOutcome, ListOutcome> {
	blocking_action(run(input, context)).await
}

async fn run(
	input: ListInput,
	context: InvocationContext<Progress>,
) -> Result<ListOutcome, ListOutcome> {
	crate::observation::read(
		None,
		input.tasks_root,
		input.state_dir,
		crate::observation::Detail::Summary,
		context,
	)
	.await
	.map(ListOutcome)
	.map_err(ListOutcome)
}
