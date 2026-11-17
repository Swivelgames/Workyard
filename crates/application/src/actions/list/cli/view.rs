use super::super::ListOutcome;
use crate::contracts::Progress;
use crate::observation::{Condition, Observation};
use crate::presentation::cli::render::{
	MessageScope, Terminal, Tone, separate, text, workspace_guidance,
};
use std::{fmt::Write, io};
use trine::{application::CliViewContext, view::View};

pub struct ListView {
	pub output: CliViewContext,
}

impl View<ListOutcome, ListOutcome, Progress> for ListView {
	type Completion = ();
	type Error = io::Error;

	async fn progress(&mut self, _: Progress) -> io::Result<()> {
		Ok(())
	}

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
		.any(|task| task.condition == Condition::NeedsAttention)
	{
		let state = observation.state_dir.as_ref().map(|path| path.as_path());
		let hint = crate::guidance::commands::workspace_status(state)
			.map(|command| format!("Inspect: {command}"))
			.unwrap_or_else(|| "Inspect with workyard status, supplying the same state directory using native-path quoting.".into());
		separate(terminal);
		terminal.hint_at(MessageScope::Workspace, &hint);
	}

	workspace_guidance(
		terminal,
		observation.guidance.as_deref(),
		observation.problem.as_ref(),
	);
}
