use super::super::ListOutcome;
use crate::contracts::Progress;
use crate::observation::{Condition, Observation};
use crate::presentation::cli::render::{
	MessageScope, TaskTag, Terminal, separate, workspace_guidance,
};
use std::io;
use trine::{application::CliViewContext, view::View};

pub struct ListView {
	pub output: CliViewContext,
}

impl View<ListOutcome, ListOutcome, Progress> for ListView {
	type Completion = ();
	type Error = io::Error;

	async fn finish(&mut self, result: &Result<ListOutcome, ListOutcome>) -> io::Result<()> {
		let outcome = match result {
			Ok(outcome) | Err(outcome) => &outcome.0,
		};
		let mut terminal = Terminal::for_output(&self.output.stdout);
		list(&mut terminal, outcome);
		self.output.stdout.write_all(terminal.output.as_bytes())?;

		Ok(())
	}
}

fn list(terminal: &mut Terminal, observation: &Observation) {
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
			let names = task
				.recorded
				.as_ref()
				.or(task.evidence.as_ref())
				.map(|saved| {
					saved
						.participants
						.iter()
						.map(|participant| participant.name.as_str())
						.collect::<Vec<_>>()
						.join(", ")
				})
				.unwrap_or_default();

			terminal.task(
				task.name.as_deref().unwrap_or("unknown identity"),
				TaskTag::observed(task),
				&names,
			);
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
		.any(|task| task.condition == Condition::NeedsAttention)
	{
		let workspace = observation
			.explicit_workspace
			.as_ref()
			.map(|path| path.as_path());
		let hint = crate::guidance::commands::workspace_status(workspace)
			.map(|command| format!("Inspect: {command}"))
			.unwrap_or_else(|| "Inspect with workyard status, supplying the selected workspace using native-path quoting.".into());
		separate(terminal);
		terminal.hint_at(MessageScope::Workspace, &hint);
	}

	workspace_guidance(
		terminal,
		observation.guidance.as_deref(),
		observation.problem.as_ref(),
		observation
			.explicit_workspace
			.as_ref()
			.map(|path| path.as_path()),
	);
}
