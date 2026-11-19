use super::{StatusInput, StatusOutcome, contract::PresentedStatus};
use crate::{contracts::Progress, execution::blocking_action};
use trine::InvocationContext;

pub async fn status(
	input: StatusInput,
	context: InvocationContext<Progress>,
) -> Result<StatusOutcome, StatusOutcome> {
	blocking_action(run(input, context)).await
}

pub(super) async fn present(
	input: StatusInput,
	context: InvocationContext<Progress>,
) -> Result<PresentedStatus, PresentedStatus> {
	let explicit_placement = input.workspace.is_some();
	let present = |outcome| PresentedStatus {
		outcome,
		explicit_placement,
	};
	status(input, context).await.map(present).map_err(present)
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
