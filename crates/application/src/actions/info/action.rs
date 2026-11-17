use super::{DiscoverySource, InfoInput, InfoOutcome};
use crate::{contracts::Progress, execution::blocking_action};
use trine::{InvocationContext, NativePath};
use workyard_core::{Control, Problem, TaskLifecycle};

pub async fn info(
	input: InfoInput,
	context: InvocationContext<Progress>,
) -> Result<InfoOutcome, InfoOutcome> {
	blocking_action(async move {
		read(input, context).map_err(|problem| InfoOutcome {
			problem: Some(problem.into()),
			..Default::default()
		})
	})
	.await
}

fn read(input: InfoInput, context: InvocationContext<Progress>) -> Result<InfoOutcome, Problem> {
	let control = Control::new(move || context.cancellation().is_cancelled());
	control.check()?;
	let cwd =
		std::env::current_dir().map_err(|error| Problem::io("Read current directory", error))?;
	let root = input.tasks_root.as_ref().map(NativePath::as_path);
	let explicit = input.state_dir.as_ref().map(NativePath::as_path);
	let Some(state) = workyard_state::find_state(root.unwrap_or(&cwd), explicit)? else {
		return Ok(InfoOutcome {
			guidance: Some(crate::guidance::workspace::missing_state(root, explicit)),
			..Default::default()
		});
	};

	let reader = workyard_state::Reader::open(&state, root)?;
	let mut result = InfoOutcome {
		tasks_root: Some(NativePath::new(reader.root()).expect("root path")),
		state_dir: Some(NativePath::new(reader.state()).expect("state path")),
		discovery: Some(if explicit.is_some() {
			DiscoverySource::Explicit
		} else {
			DiscoverySource::Ancestor
		}),
		..Default::default()
	};
	for snapshot in reader.tasks(None)? {
		control.check()?;
		if snapshot.busy {
			result.busy_tasks += 1;
		}
		if !snapshot.problems.is_empty() || snapshot.task.is_none() || snapshot.pending.is_some() {
			result.unresolved_records += 1;
		}
		match snapshot.task.as_ref().map(|task| task.lifecycle) {
			Some(TaskLifecycle::Active) => result.active_tasks += 1,
			Some(TaskLifecycle::Retired) => result.retired_tasks += 1,
			None => {}
		}
	}
	Ok(result)
}
