//! Reconcile only workspace placement. Task lifecycle operations run via named recovery.
mod activation;
mod inspect;
mod plan;
mod selected;
use super::super::{RecoverInput, workspace::WorkspaceRecovery};
use crate::contracts::Progress;
use plan::Plan;
use trine::{InvocationContext, NativePath};
use workyard_core::{Control, Problem, ProblemCode};
use workyard_git::Runner;
use workyard_state::{Reader, Store};

pub(super) async fn run(
	input: RecoverInput,
	context: InvocationContext<Progress>,
) -> Result<WorkspaceRecovery, WorkspaceRecovery> {
	let reject = |p| WorkspaceRecovery::default().failed(p);
	if input.undo || input.force || input.yes {
		return Err(reject(Problem::new(
			ProblemCode::InvalidInput,
			"Task recovery options require a task name.",
		)));
	}
	let state = crate::workspace::required(input.workspace.as_ref().map(NativePath::as_path))
		.map_err(reject)?;
	let reader = Reader::open(&state, None).map_err(reject)?;
	let store = Store::open_existing(&state, None).map_err(reject)?;
	let runner = Runner::default();
	let cancellation = context.cancellation().clone();
	let control = Control::new(move || cancellation.is_cancelled());
	let mut report = WorkspaceRecovery::at(state.parent().expect("workspace marker"));
	let plan = Plan::inspect(&reader, &runner, &control, &mut report)
		.await
		.map_err(reject)?;

	let activate = match activation::choose(&input, &report, &context).await {
		Ok(activate) => activate,
		Err(problem) => return Err(report.failed(problem)),
	};
	let changed = plan
		.apply(&store, &runner, &control, activate, &mut report)
		.await;
	let remembered = match store.remember_workspace(&control).await {
		Ok(changed) => changed,
		Err(problem) => return Err(report.failed(problem)),
	};
	report.explicit_workspace = input.workspace;
	report.finish(changed || remembered)
}
