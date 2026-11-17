use super::{AmendInput, AmendOutcome};
use crate::{
	contracts::Progress,
	execution::{Events, blocking_action},
};
use trine::InvocationContext;
use workyard_core::{Control, OperationOutcome, ProblemCode};

pub async fn amend(
	input: AmendInput,
	context: InvocationContext<Progress>,
) -> Result<AmendOutcome, AmendOutcome> {
	blocking_action(run(input, context)).await
}

async fn run(
	input: AmendInput,
	context: InvocationContext<Progress>,
) -> Result<AmendOutcome, AmendOutcome> {
	let name = input.task.clone();
	let rejected = |problem| AmendOutcome::rejected(name.clone(), problem);

	let cancellation = context.cancellation().clone();
	let control = Control::new(move || cancellation.is_cancelled());
	control.check().map_err(rejected)?;

	let prepared = input.prepare().map_err(rejected)?;
	let store = workyard_state::Store::open_existing(&prepared.state, prepared.root.as_deref())
		.map_err(rejected)?;

	let result = workyard_core::amend(
		prepared.request,
		&workyard_git::Runner::default(),
		&control,
		&Events(context, "amend"),
		&store,
	)
	.await;

	match result {
		Ok(result) => {
			let outcome = AmendOutcome::from_result(result);
			if outcome.operation.outcome == OperationOutcome::Failed {
				Err(outcome)
			} else {
				Ok(outcome)
			}
		}
		Err(problem) if problem.code == ProblemCode::Cancelled => Ok(rejected(problem)),
		Err(problem) => Err(rejected(problem)),
	}
}
