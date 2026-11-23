use super::super::StatusOutcome;
use crate::contracts::Progress;
use crate::presentation::cli::render::{Terminal, Tone, separate, workspace_guidance};
use std::{fmt::Write, io};
use trine::{application::CliViewContext, view::View};

pub struct StatusView {
	pub output: CliViewContext,
}

impl View<StatusOutcome, StatusOutcome, Progress> for StatusView {
	type Completion = ();
	type Error = io::Error;

	async fn finish(&mut self, result: &Result<StatusOutcome, StatusOutcome>) -> io::Result<()> {
		let outcome = match result {
			Ok(outcome) | Err(outcome) => outcome,
		};
		let mut terminal = Terminal::for_output(&self.output.stdout);
		status(&mut terminal, outcome);
		self.output.stdout.write_all(terminal.output.as_bytes())?;

		Ok(())
	}
}

fn status(terminal: &mut Terminal, outcome: &StatusOutcome) {
	let observation = &outcome.0;
	let tasks_root = observation.tasks_root.as_ref().map(|path| path.as_path());
	let workspace = observation
		.state_dir
		.as_ref()
		.and_then(|path| path.as_path().parent());
	let explicit_workspace = observation
		.explicit_workspace
		.as_ref()
		.map(|path| path.as_path());

	if !observation.tasks.is_empty() {
		let title = terminal.style.paint("Task status", Tone::Section);
		writeln!(terminal.output, "  {title}").expect("string output");
	}

	for task in &observation.tasks {
		separate(terminal);
		crate::presentation::cli::render::task::render(
			terminal,
			task,
			workspace,
			tasks_root,
			explicit_workspace,
		);
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
