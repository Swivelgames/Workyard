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

	let prepared = input.prepare(&control).await.map_err(rejected)?;
	let store = workyard_state::Store::open_existing(&prepared.state, None).map_err(rejected)?;

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
			let registration =
				crate::projects::register(&store, &result.admitted_roots, &control).await;
			let mut outcome = AmendOutcome::from_result(result);
			outcome.registration_problem = registration.map(Into::into);
			if outcome.registration_problem.is_some()
				|| outcome.operation.outcome == OperationOutcome::Failed
			{
				Err(outcome)
			} else {
				Ok(outcome)
			}
		}
		Err(problem) if problem.code == ProblemCode::Cancelled => Ok(rejected(problem)),
		Err(problem) => Err(rejected(problem)),
	}
}
