use super::{InitInput, InitOutcome};
use crate::{
	contracts::{ProblemOutcome, Progress},
	execution::blocking_action,
};
use std::path::Path;
use trine::{InvocationContext, NativePath};
use workyard_core::Control;

pub async fn init(
	input: InitInput,
	context: InvocationContext<Progress>,
) -> Result<InitOutcome, ProblemOutcome> {
	blocking_action(async move {
		let control = Control::new(move || context.cancellation().is_cancelled());
		control.check()?;

		let directory = input
			.directory
			.as_ref()
			.map(NativePath::as_path)
			.unwrap_or(Path::new("."));
		let workspace = workyard_state::canonical_future(directory)?;
		let tasks = workyard_state::initialize_workspace(
			&workspace,
			input.tasks.as_ref().map(NativePath::as_path),
		)?;

		Ok(InitOutcome {
			workspace: NativePath::new(workspace).expect("workspace path"),
			tasks_root: NativePath::new(tasks).expect("tasks root path"),
		})
	})
	.await
}
