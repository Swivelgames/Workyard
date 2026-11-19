use trine::InvocationContext;
use workyard_core::{AmendState, Control, ProblemCode, RecoveryState};
use workyard_git::Runner;
use workyard_state::Store;

use crate::{contracts::Progress, execution::blocking_action};

use super::{RecoverInput, RecoverOutcome};

mod amendment;
mod creation;
mod removal;
mod workspace;

pub async fn recover(
	mut input: RecoverInput,
	context: InvocationContext<Progress>,
) -> Result<super::RecoveryOutcome, super::RecoveryOutcome> {
	use super::RecoveryOutcome::{Task, Workspace};
	blocking_action(async move {
		if let Some(name) = input.task.take() {
			run(name, input, context)
				.await
				.map(|r| Task(Box::new(r)))
				.map_err(|r| Task(Box::new(r)))
		} else {
			workspace::run(input, context)
				.await
				.map(Workspace)
				.map_err(Workspace)
		}
	})
	.await
}

async fn run(
	name: String,
	input: RecoverInput,
	context: InvocationContext<Progress>,
) -> Result<RecoverOutcome, RecoverOutcome> {
	let rejected = |problem| RecoverOutcome::rejected(name.clone(), problem);
	if input.activate_here {
		return Err(rejected(workyard_core::Problem::new(
			ProblemCode::InvalidInput,
			"--activate-here applies to workspace recovery. Omit the task name.",
		)));
	}
	workyard_core::validate_task(&name).map_err(rejected)?;

	let cancellation = context.cancellation().clone();
	let control = Control::new(move || cancellation.is_cancelled());
	control.check().map_err(rejected)?;

	let state =
		crate::workspace::required(input.workspace.as_ref().map(trine::NativePath::as_path))
			.map_err(rejected)?;
	let store = Store::open_existing(&state, None).map_err(rejected)?;
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
		return removal::run(name, input, context, &runner, &control, &store).await;
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
