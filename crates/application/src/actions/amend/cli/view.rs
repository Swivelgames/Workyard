use super::super::AmendOutcome;
use crate::{
	contracts::Progress,
	presentation::cli::render::{MessageScope, Terminal, text},
};
use std::io;
use trine::{application::CliViewContext, view::View};
use workyard_core::OperationOutcome;

pub struct AmendView {
	pub output: CliViewContext,
}

impl View<AmendOutcome, AmendOutcome, Progress> for AmendView {
	type Completion = ();
	type Error = io::Error;

	async fn progress(&mut self, progress: Progress) -> io::Result<()> {
		let mut terminal = Terminal::for_output(&self.output.stderr);
		terminal.progress(&progress);
		self.output.stderr.write_all(terminal.output.as_bytes())
	}

	async fn finish(&mut self, result: &Result<AmendOutcome, AmendOutcome>) -> io::Result<()> {
		let outcome = match result {
			Ok(outcome) | Err(outcome) => outcome,
		};
		let succeeded = outcome.operation.outcome == OperationOutcome::Succeeded;
		let unchanged = outcome
			.changes
			.iter()
			.all(|change| change.kind == "unchanged");
		let mut terminal = Terminal::for_output(&self.output.stdout);
		if !outcome.pull_failures.is_empty() && outcome.problem.is_none() {
			terminal.completed_with_pull_failures("Amendment", &outcome.task);
		} else if succeeded && unchanged {
			terminal.heading(&format!("No changes needed: {}", text(&outcome.task)));
		} else {
			terminal.heading(&format!(
				"Amendment {}: {}",
				outcome.operation.outcome.as_str(),
				text(&outcome.task)
			));
		}
		if !succeeded && !unchanged {
			let changed = outcome
				.changes
				.iter()
				.filter(|change| change.kind != "unchanged")
				.map(|change| format!("{} ({})", text(&change.participant), change.kind))
				.collect::<Vec<_>>()
				.join(", ");
			terminal.hint_at(
				MessageScope::Workspace,
				&format!("Changes retained: {changed}"),
			);
		}
		terminal.pull_failures(&outcome.pull_failures);
		if let Some(problem) = &outcome.registration_problem {
			terminal.problem(problem);
		}
		if let Some(problem) = &outcome.problem {
			terminal.problem_at(MessageScope::Workspace, problem);
		}
		self.output.stdout.write_all(terminal.output.as_bytes())?;
		Ok(())
	}
}
