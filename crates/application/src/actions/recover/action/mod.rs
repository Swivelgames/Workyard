use trine::InvocationContext;
use workyard_core::{AmendState, Control, ProblemCode, RecoveryState};
use workyard_git::Runner;
use workyard_state::Store;

use crate::{contracts::Progress, execution::blocking_action};

use super::{RecoverInput, RecoverOutcome};

mod amendment;
mod creation;
mod removal;

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
	let guard = store.admit(&name).map_err(rejected)?;
	let record = store.load(&guard, &name).map_err(rejected)?;
	let removing = record.task.removal.is_some();
	let amending = record.task.operation.kind == Some(workyard_core::OperationKind::Amend)
		|| record.task.operation.continues == Some(workyard_core::OperationKind::Amend);
	let amendment = if amending && !removing {
		store.amend_request(&guard).map_err(rejected)?
	} else {
		None
	};
	drop(guard);

	if removing {
		return removal::run(input, context, &runner, &control, &store).await;
	}

	if input.undo || input.force || input.yes {
		return Err(rejected(workyard_core::Problem::new(
			ProblemCode::InvalidInput,
			"--undo, --yes and --force currently apply only to removal recovery.",
		)));
	}

	if let Some(request) = amendment {
		return amendment::run(
			request,
			&name,
			&input.accept_pull,
			&runner,
			&control,
			context,
			&store,
		)
		.await;
	}

	creation::run(
		&name,
		&input.accept_pull,
		&runner,
		&control,
		context,
		&store,
	)
	.await
}
