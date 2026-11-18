use super::super::contract::PresentedStatus;
use crate::contracts::Progress;
use crate::presentation::cli::render::{Terminal, separate, workspace_guidance};
use std::io;
use trine::{application::CliViewContext, view::View};

pub struct StatusView {
	pub output: CliViewContext,
}

impl View<PresentedStatus, PresentedStatus, Progress> for StatusView {
	type Completion = ();
	type Error = io::Error;

	async fn progress(&mut self, _: Progress) -> io::Result<()> {
		Ok(())
	}

	async fn finish(
		&mut self,
		result: &Result<PresentedStatus, PresentedStatus>,
	) -> io::Result<()> {
		let outcome = match result {
			Ok(outcome) | Err(outcome) => outcome,
		};
		let mut terminal = Terminal::for_output(&self.output.stdout);
		status(&mut terminal, outcome);
		self.output.stdout.write_all(terminal.output.as_bytes())?;

		Ok(())
	}
}

fn status(terminal: &mut Terminal, outcome: &PresentedStatus) {
	let observation = &outcome.outcome.0;
	let root = observation.tasks_root.as_ref().map(|path| path.as_path());
	let recovery_state = observation
		.state_dir
		.as_ref()
		.filter(|_| outcome.explicit_placement)
		.map(|path| path.as_path());

	for task in &observation.tasks {
		separate(terminal);
		crate::presentation::cli::render::task::render(terminal, task, root, recovery_state, true);
	}
	workspace_guidance(
		terminal,
		observation.guidance.as_deref(),
		observation.problem.as_ref(),
	);
}
