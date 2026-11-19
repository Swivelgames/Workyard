use super::super::{DiscoverySource, InfoOutcome};
use crate::{
	contracts::Progress,
	presentation::cli::render::{MessageScope, Terminal},
};
use std::{fmt::Write, io};
use trine::{application::CliViewContext, view::View};

pub struct InfoView {
	pub output: CliViewContext,
}

impl View<InfoOutcome, InfoOutcome, Progress> for InfoView {
	type Completion = ();
	type Error = io::Error;

	async fn progress(&mut self, _: Progress) -> io::Result<()> {
		Ok(())
	}

	async fn finish(&mut self, result: &Result<InfoOutcome, InfoOutcome>) -> io::Result<()> {
		let outcome = match result {
			Ok(outcome) | Err(outcome) => outcome,
		};
		let mut terminal = Terminal::for_output(&self.output.stdout);
		if let (Some(workspace), Some(tasks), Some(state)) =
			(&outcome.workspace, &outcome.tasks_root, &outcome.state_dir)
		{
			let root = workspace.as_path();
			let label = terminal.paths.root(root);
			let tasks = terminal.paths.within(root, tasks.as_path());
			let state = terminal.paths.within(root, state.as_path());
			writeln!(terminal.output, "Workspace: {label}").expect("string output");
			writeln!(terminal.output, "Tasks dir: {tasks}").expect("string output");
			writeln!(terminal.output, "State:     {state}").expect("string output");
		}
		if let Some(source) = outcome.discovery {
			let source = match source {
				DiscoverySource::Explicit => "explicit --workspace",
				DiscoverySource::Ancestor => "ancestor .workyard search",
			};
			writeln!(terminal.output, "Discovery: {source}").expect("string output");
			writeln!(
				terminal.output,
				"Tasks: {} active, {} retired",
				outcome.active_tasks, outcome.retired_tasks
			)
			.expect("string output");
			if outcome.unresolved_records > 0 {
				let state = outcome.state_dir.as_ref().map(|path| path.as_path());
				let command = crate::guidance::commands::workspace_status(state)
					.unwrap_or_else(|| "workyard status with the same workspace".into());
				writeln!(
					terminal.output,
					"Unresolved records: {} (inspect with {command})",
					outcome.unresolved_records
				)
				.expect("string output");
			}
			if outcome.busy_tasks > 0 {
				writeln!(terminal.output, "Locked tasks: {}", outcome.busy_tasks)
					.expect("string output");
			}
		}
		if let Some(guidance) = &outcome.guidance {
			for line in guidance.lines() {
				terminal.hint_at(MessageScope::Workspace, line);
			}
		}
		if let Some(problem) = &outcome.problem {
			terminal.problem_at(MessageScope::Workspace, problem);
		}
		self.output.stdout.write_all(terminal.output.as_bytes())?;
		Ok(())
	}
}
