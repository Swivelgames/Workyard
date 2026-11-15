//! Small read-only components; list and status deliberately choose different detail.
mod participants;
mod task;

use super::Observation;
use crate::terminal::{MessageScope, Terminal, Tone, text};
use std::fmt::Write;

pub(crate) fn list(terminal: &mut Terminal, observation: &Observation) {
	for (lifecycle, heading) in [
		(Some("active"), "Active tasks"),
		(Some("retired"), "Recently closed tasks"),
		(None, "Unresolved records"),
	] {
		let tasks: Vec<_> = observation
			.tasks
			.iter()
			.filter(|task| task.lifecycle == lifecycle)
			.collect();
		if tasks.is_empty() {
			continue;
		}
		separate(terminal);
		terminal.heading(heading);
		for task in tasks {
			let label = if lifecycle == Some("retired") {
				"retired"
			} else {
				task.condition.label()
			};
			let names = task
				.recorded
				.as_ref()
				.or(task.evidence.as_ref())
				.map(|saved| {
					saved
						.participants
						.iter()
						.map(|participant| text(&participant.name))
						.collect::<Vec<_>>()
						.join(", ")
				})
				.unwrap_or_default();
			writeln!(
				terminal.output,
				"    {} [{}]  {}",
				terminal.style.paint(
					&text(task.name.as_deref().unwrap_or("unknown identity")),
					Tone::Identity
				),
				text(label),
				names
			)
			.expect("string output");
			if task.recorded.is_none() {
				terminal.note(&format!(
					"State record: {}",
					terminal.paths.root(task.record.as_path())
				));
			}
		}
	}
	if observation
		.tasks
		.iter()
		.any(|task| task.condition == super::Condition::NeedsAttention)
	{
		let state = observation.state_dir.as_ref().map(|path| path.as_path());
		let hint = crate::command_hint::workspace_status(state)
			.map(|command| format!("Inspect: {command}"))
			.unwrap_or_else(|| "Inspect with workyard status, supplying the same state directory using native-path quoting.".into());
		separate(terminal);
		terminal.hint_at(MessageScope::Workspace, &hint);
	}

	guidance(terminal, observation);
}

pub(crate) fn status(terminal: &mut Terminal, observation: &Observation) {
	let root = observation.tasks_root.as_ref().map(|path| path.as_path());
	for task in &observation.tasks {
		separate(terminal);
		task::render(terminal, task, root, true);
	}
	guidance(terminal, observation);
}

fn separate(terminal: &mut Terminal) {
	if !terminal.output.is_empty() && !terminal.output.ends_with("\n\n") {
		terminal.output.push('\n');
	}
}

fn guidance(terminal: &mut Terminal, observation: &Observation) {
	if observation.guidance.is_none() && observation.problem.is_none() {
		return;
	}
	separate(terminal);
	if let Some(guidance) = &observation.guidance {
		for line in guidance.lines() {
			terminal.hint_at(MessageScope::Workspace, line);
		}
	}
	if let Some(problem) = &observation.problem {
		terminal.problem_at(MessageScope::Workspace, problem);
	}
}
