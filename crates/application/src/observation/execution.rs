use super::{Observation, guidance};
use crate::common::Progress;
use std::path::Path;
use trine::{InvocationContext, NativePath};
use workyard_core::{Control, Problem};

#[derive(Clone, Copy)]
pub(crate) enum Detail {
	Summary,
	Git,
}

pub(crate) async fn read(
	task: Option<String>,
	root: Option<NativePath>,
	state: Option<NativePath>,
	detail: Detail,
	context: InvocationContext<Progress>,
) -> Result<Observation, Observation> {
	let result = observe(
		task.as_deref(),
		root.as_ref().map(NativePath::as_path),
		state.as_ref().map(NativePath::as_path),
		detail,
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
	root: Option<&Path>,
	explicit: Option<&Path>,
	detail: Detail,
	context: InvocationContext<Progress>,
) -> Result<Observation, Problem> {
	if let Some(task) = task {
		workyard_core::validate_task(task)?;
	}

	let cancellation = context.cancellation().clone();
	let control = Control::new(move || cancellation.is_cancelled());
	control.check()?;

	let cwd = std::env::current_dir().map_err(|e| Problem::io("Read current directory", e))?;
	let Some(state) = workyard_state::find_state(root.unwrap_or(&cwd), explicit)? else {
		return Ok(Observation {
			guidance: Some(guidance::missing_state(root, explicit)),
			..Observation::default()
		});
	};

	let reader = workyard_state::Reader::open(&state, root)?;
	let mut observation = Observation {
		selected_task: task.map(str::to_owned),
		state_dir: Some(NativePath::new(reader.state()).expect("state path")),
		tasks_root: Some(NativePath::new(reader.root()).expect("root path")),
		..Observation::default()
	};

	for snapshot in reader.tasks(task)? {
		control.check()?;
		let report: super::TaskReport = if matches!(detail, Detail::Git) {
			workyard_core::inspect(snapshot, &workyard_git::Runner::default(), &control)
				.await?
				.into()
		} else {
			snapshot.into()
		};
		observation.tasks.push(report);
	}

	if observation.tasks.is_empty() {
		observation.guidance = Some(guidance::empty_workspace(reader.state(), reader.root()));
	}

	Ok(observation)
}
