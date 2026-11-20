use trine::{InvocationContext, NativePath};
use workyard_core::{Control, Problem, ProblemCode, RemovalOptions};
use workyard_git::Runner;
use workyard_state::Store;

use crate::{contracts::Progress, execution::blocking_action};

use super::{RecoverInput, RecoverOutcome, RecoveryRequest};

mod amendment;
mod creation;
mod removal;
mod route;
mod workspace;

use route::{Authority, Route};

pub async fn recover(
	input: RecoverInput,
	context: InvocationContext<Progress>,
) -> Result<super::RecoveryOutcome, super::RecoveryOutcome> {
	execute(input.into(), context).await
}

pub(super) async fn execute(
	request: RecoveryRequest,
	context: InvocationContext<Progress>,
) -> Result<super::RecoveryOutcome, super::RecoveryOutcome> {
	use super::RecoveryOutcome::{Task, Workspace};
	blocking_action(async move {
		let (name, workspace, authority) = match request {
			RecoveryRequest::General(mut input) => {
				let Some(name) = input.task.take() else {
					return workspace::run(input, context)
						.await
						.map(Workspace)
						.map_err(Workspace);
				};
				if input.activate_here {
					let problem = Problem::new(
						ProblemCode::InvalidInput,
						"--activate-here applies to workspace recovery. Omit the task name.",
					);
					return Err(Task(Box::new(RecoverOutcome::rejected(name, problem))));
				}
				let options = RemovalOptions {
					undo: input.undo,
					yes: input.yes,
					force: input.force,
				};
				(name, input.workspace, Authority::General(options))
			}
			RecoveryRequest::Participants(named) => {
				(named.task, named.workspace, Authority::Participants)
			}
		};
		run(name, workspace, authority, context)
			.await
			.map(|r| Task(Box::new(r)))
			.map_err(|r| Task(Box::new(r)))
	})
	.await
}

async fn run(
	name: String,
	workspace: Option<NativePath>,
	authority: Authority,
	context: InvocationContext<Progress>,
) -> Result<RecoverOutcome, RecoverOutcome> {
	let rejected = |problem| RecoverOutcome::rejected(name.clone(), problem);
	workyard_core::validate_task(&name).map_err(rejected)?;

	let cancellation = context.cancellation().clone();
	let control = Control::new(move || cancellation.is_cancelled());
	control.check().map_err(rejected)?;

	let state = crate::workspace::required(workspace.as_ref().map(NativePath::as_path))
		.map_err(rejected)?;
	let store = Store::open_existing(&state, None).map_err(rejected)?;
	let runner = Runner::default();

	// Participant-only recovery is authorized again under the core mutation lock.
	match Route::select(&name, authority, &store).map_err(rejected)? {
		Route::Creation(scope) => {
			creation::run(&name, scope, &runner, &control, context, &store).await
		}
		Route::Amendment(scope) => {
			amendment::run(&name, scope, &runner, &control, context, &store).await
		}
		Route::Removal(options) => {
			removal::run(&name, options, &runner, &control, context, &store).await
		}
	}
}
