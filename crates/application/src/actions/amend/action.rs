use super::{AmendInput, AmendOutcome};
use crate::{
	contracts::Progress,
	execution::{
		blocking_action,
		removal::{RemovalEvents, RemovalRequest},
	},
};
use trine::InvocationContext;
use workyard_core::{Control, OperationOutcome};

/// Apply the requested membership changes to a managed task.
pub async fn amend(
	input: AmendInput,
	context: InvocationContext<Progress>,
) -> Result<AmendOutcome, AmendOutcome> {
	execute(input.into(), context).await
}

pub(super) async fn execute(
	request: RemovalRequest<AmendInput>,
	context: InvocationContext<Progress>,
) -> Result<AmendOutcome, AmendOutcome> {
	blocking_action(run(request, context)).await
}

async fn run(
	request: RemovalRequest<AmendInput>,
	context: InvocationContext<Progress>,
) -> Result<AmendOutcome, AmendOutcome> {
	let input = request.input;
	let name = input.task.clone();
	let rejected = |problem| AmendOutcome::rejected(name.clone(), problem);

	let cancellation = context.cancellation().clone();
	let control = Control::new(move || cancellation.is_cancelled());
	control.check().map_err(rejected)?;

	let explicit_workspace = input.workspace.is_some();
	let prepared = input.prepare(&control).await.map_err(rejected)?;
	let store = workyard_state::Store::open_existing(&prepared.state, None).map_err(rejected)?;

	let observer = RemovalEvents::new(context, "amend", name.clone())
		.with_consent(request.consent, &prepared.state);
	let result = workyard_core::amend(
		prepared.request,
		&workyard_git::Runner::default(),
		&control,
		&observer,
		&store,
	)
	.await;

	let mut outcome = match result {
		Ok(result) => {
			let registration =
				crate::projects::register(&store, &result.admitted_roots, &control).await;
			let mut outcome = AmendOutcome::from_result(result);
			outcome.registration_problem = registration.map(Into::into);
			outcome
		}
		Err(problem) => rejected(problem),
	};
	outcome.removal = observer.into_report();
	outcome.guide_retirement(&prepared.state, explicit_workspace);

	if outcome.registration_problem.is_some()
		|| outcome.operation.outcome == OperationOutcome::Failed
	{
		Err(outcome)
	} else {
		Ok(outcome)
	}
}
