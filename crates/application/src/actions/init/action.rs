use super::{InitInput, InitOutcome, skill};
use crate::{
	contracts::{ProblemOutcome, Progress},
	execution::blocking_action,
};
use std::path::Path;
use trine::{InvocationContext, NativePath};
use workyard_core::{Control, Problem};

/// Initialize a workspace with the requested tasks directory.
pub async fn init(
	input: InitInput,
	context: InvocationContext<Progress>,
) -> Result<InitOutcome, ProblemOutcome> {
	blocking_action(async move {
		let cancellation = context.cancellation().clone();
		let control = Control::new({
			let cancellation = cancellation.clone();
			move || cancellation.is_cancelled()
		});

		let directory = input
			.directory
			.as_ref()
			.map(NativePath::as_path)
			.unwrap_or(Path::new("."));

		let workspace = workyard_state::canonical_future(directory)?;
		let requested_tasks = input.tasks.as_ref().map(NativePath::as_path);
		if let Some(tasks) = requested_tasks {
			skill::check_tasks(&workspace, tasks)?;
		}

		let tasks = workyard_state::initialize_workspace_with_control(
			&workspace,
			requested_tasks,
			&control,
		)
		.await?;

		skill::install(&workspace).map_err(|error| {
			Problem::io(
				format!("Workspace is ready. Could not install {}", skill::LOCATION),
				error,
			)
			.remedy("Check the skill path and rerun workyard init for the same workspace.")
		})?;

		Ok(InitOutcome {
			cancellation,
			workspace: NativePath::new(workspace).expect("workspace path"),
			tasks_root: NativePath::new(tasks).expect("tasks root path"),
		})
	})
	.await
}
