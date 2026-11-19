use super::super::InitOutcome;
use crate::{
	contracts::{ProblemOutcome, Progress},
	presentation::cli::render::{MessageScope, Terminal},
};
use std::{fmt::Write, io};
use trine::{application::CliViewContext, view::View};

pub struct InitView {
	pub output: CliViewContext,
}

impl View<InitOutcome, ProblemOutcome, Progress> for InitView {
	type Completion = ();
	type Error = io::Error;

	async fn progress(&mut self, _: Progress) -> io::Result<()> {
		Ok(())
	}

	async fn finish(&mut self, result: &Result<InitOutcome, ProblemOutcome>) -> io::Result<()> {
		let mut terminal = Terminal::for_output(&self.output.stdout);
		match result {
			Ok(outcome) => {
				let workspace = outcome.workspace.as_path();
				let tasks = terminal
					.paths
					.within(workspace, outcome.tasks_root.as_path());
				writeln!(
					terminal.output,
					"Workspace ready at {}",
					terminal.paths.root(workspace)
				)
				.expect("string output");
				writeln!(terminal.output, "    Tasks: {tasks}").expect("string output");
			}
			Err(problem) => terminal.problem_at(MessageScope::Workspace, problem),
		}

		self.output.stdout.write_all(terminal.output.as_bytes())?;
		Ok(())
	}
}
