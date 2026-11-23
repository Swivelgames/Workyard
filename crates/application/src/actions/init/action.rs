use super::{InitInput, InitOutcome, skill};
use crate::{
	contracts::{ProblemOutcome, Progress},
	execution::blocking_action,
};
use std::path::Path;
use trine::{InvocationContext, NativePath};
use workyard_core::{Control, Problem, ProblemCode};
use workyard_state::InitOptions;

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
		if input.tasks.is_some() && input.detached_tasks.is_some() {
			return Err(Problem::new(
				ProblemCode::InvalidInput,
				"Use either --tasks or --detached-tasks.",
			)
			.into());
		}
		let requested_tasks = input
			.tasks
			.as_ref()
			.or(input.detached_tasks.as_ref())
			.map(NativePath::as_path);
		if let Some(tasks) = requested_tasks {
			skill::check_tasks(&workspace, tasks)?;
		}

		let track: Vec<_> = input.track.iter().map(NativePath::as_path).collect();
		let tasks = workyard_state::initialize_workspace_with_options(
			&workspace,
			InitOptions {
				tasks: requested_tasks,
				detached_tasks: input.detached_tasks.is_some(),
				tracked_only: input.tracked_only,
				track: &track,
			},
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
