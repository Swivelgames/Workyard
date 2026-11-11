//! Views compose these facts without discovering state or repairing worktrees.
mod participants;
mod task;

use super::Observation;
use crate::terminal::{MessageScope, Terminal};

pub(crate) fn workspace(terminal: &mut Terminal, observation: &Observation, title: &str) {
	terminal.heading(title);
	let root = observation.tasks_root.as_ref().map(|path| path.as_path());
	terminal.workspace(
		root,
		observation.state_dir.as_ref().map(|path| path.as_path()),
	);
	let detail = observation.selected_task.is_some();

	for (known, heading) in [(true, "Active tasks"), (false, "Unresolved records")] {
		let tasks: Vec<_> = observation
			.tasks
			.iter()
			.filter(|task| task.lifecycle.is_some() == known)
			.collect();
		if tasks.is_empty() {
			continue;
		}

		if !terminal.output.ends_with("\n\n") {
			terminal.output.push('\n');
		}
		terminal.heading(heading);

		for task in tasks {
			terminal.output.push('\n');
			task::render(terminal, task, root, detail);
		}
	}

	if observation.guidance.is_none() && observation.problem.is_none() {
		return;
	}

	if !terminal.output.ends_with("\n\n") {
		terminal.output.push('\n');
	}
	if let Some(guidance) = &observation.guidance {
		for line in guidance.lines() {
			terminal.hint_at(MessageScope::Workspace, line);
		}
	}
	if let Some(problem) = &observation.problem {
		terminal.problem_at(MessageScope::Workspace, problem);
	}
}
