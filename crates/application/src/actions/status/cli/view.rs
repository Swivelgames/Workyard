use super::super::StatusOutcome;
use crate::contracts::Progress;
use crate::observation::Observation;
use crate::presentation::cli::render::{Terminal, separate, workspace_guidance};
use std::io;
use trine::{application::CliViewContext, view::View};

pub struct StatusView {
	pub output: CliViewContext,
}

impl View<StatusOutcome, StatusOutcome, Progress> for StatusView {
	type Completion = ();
	type Error = io::Error;

	async fn progress(&mut self, _: Progress) -> io::Result<()> {
		Ok(())
	}

	async fn finish(&mut self, result: &Result<StatusOutcome, StatusOutcome>) -> io::Result<()> {
		let outcome = match result {
			Ok(outcome) | Err(outcome) => &outcome.0,
		};
		let mut terminal = Terminal::for_output(&self.output.stdout);
		status(&mut terminal, outcome);
		self.output.stdout.write_all(terminal.output.as_bytes())?;

		Ok(())
	}
}

fn status(terminal: &mut Terminal, observation: &Observation) {
	let root = observation.tasks_root.as_ref().map(|path| path.as_path());
	for task in &observation.tasks {
		separate(terminal);
		crate::presentation::cli::render::task::render(terminal, task, root, true);
	}
	workspace_guidance(
		terminal,
		observation.guidance.as_deref(),
		observation.problem.as_ref(),
	);
}
