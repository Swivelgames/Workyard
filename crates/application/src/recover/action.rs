use trine::InvocationContext;
use workyard_core::{Control, ProblemCode, TaskStatus};
use workyard_git::Runner;
use workyard_state::Store;

use crate::{
	common::Progress,
	invocation::{Events, blocking_action},
};

use super::{RecoverInput, RecoverOutcome};

pub async fn recover(
	input: RecoverInput,
	context: InvocationContext<Progress>,
) -> Result<RecoverOutcome, RecoverOutcome> {
	blocking_action(run(input, context)).await
}

async fn run(
	input: RecoverInput,
	context: InvocationContext<Progress>,
) -> Result<RecoverOutcome, RecoverOutcome> {
	let name = input.task.clone();
	let rejected = |problem| RecoverOutcome::rejected(name.clone(), problem);
	workyard_core::validate_task(&name).map_err(rejected)?;

	let cancellation = context.cancellation().clone();
	let control = Control::new(move || cancellation.is_cancelled());
	control.check().map_err(rejected)?;

	let location = input.location().map_err(rejected)?;
	let store =
		Store::open_existing(&location.state, location.root.as_deref()).map_err(rejected)?;
	let runner = Runner::default();
	let accepted = input.accept_pull;

	let result = workyard_core::recover(
		&name,
		&accepted,
		&runner,
		&control,
		&Events(context),
		&store,
	)
	.await;
	match result {
		Ok((task, record)) => {
			let outcome = RecoverOutcome::from_task(task, record);
			if outcome.status == TaskStatus::Failed {
				Err(outcome)
			} else {
				Ok(outcome)
			}
		}
		Err(problem) if problem.code == ProblemCode::Cancelled => Ok(rejected(problem)),
		Err(problem) => Err(rejected(problem)),
	}
}
