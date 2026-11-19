use super::{Observation, Selection};
use crate::{contracts::Progress, guidance::workspace as guidance};
use std::path::Path;
use trine::{InvocationContext, NativePath};
use workyard_core::{Control, Problem};

pub(crate) async fn read(
	task: Option<String>,
	workspace: Option<NativePath>,
	selection: Selection,
	context: InvocationContext<Progress>,
) -> Result<Observation, Observation> {
	let result = observe(
		task.as_deref(),
		workspace.as_ref().map(NativePath::as_path),
		selection,
		context,
	)
	.await;
	result.map_err(|problem| Observation {
		problem: Some(problem.into()),
		..Observation::default()
	})
}

async fn observe(
	task: Option<&str>,
	workspace: Option<&Path>,
	selection: Selection,
	context: InvocationContext<Progress>,
) -> Result<Observation, Problem> {
	if let Some(task) = task {
		workyard_core::validate_task(task)?;
	}

	let cancellation = context.cancellation().clone();
	let control = Control::new(move || cancellation.is_cancelled());
	control.check()?;

	let cwd = std::env::current_dir().map_err(|e| Problem::io("Read current directory", e))?;
	let Some(reader) = crate::workspace::reader(&cwd, workspace)? else {
		return Ok(Observation {
			guidance: Some(guidance::missing_state(workspace)),
			..Observation::default()
		});
	};

	let mut observation = Observation {
		selected_task: task.map(str::to_owned),
		state_dir: Some(NativePath::new(reader.state()).expect("state path")),
		tasks_root: Some(NativePath::new(reader.root()).expect("root path")),
		..Observation::default()
	};

	let snapshots = selection.snapshots(&reader, task)?;

	for snapshot in snapshots {
		control.check()?;
		let report: super::TaskReport = if !snapshot
			.task
			.as_ref()
			.is_some_and(|task| task.lifecycle == workyard_core::TaskLifecycle::Retired)
		{
			workyard_core::inspect(snapshot, &workyard_git::Runner::default(), &control)
				.await?
				.into()
		} else {
			snapshot.into()
		};
		observation.tasks.push(report);
	}

	if observation.tasks.is_empty() {
		observation.guidance = Some(match selection {
			Selection::Active => "No active tasks found.".into(),
			Selection::Inventory => guidance::empty_workspace(workspace),
		});
	}

	Ok(observation)
}
