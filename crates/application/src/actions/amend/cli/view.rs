use super::super::AmendOutcome;
use crate::{
	contracts::Progress,
	presentation::cli::render::{MessageScope, Terminal, separate, text},
};
use std::io;
use trine::{application::CliViewContext, view::View};
use workyard_core::OperationOutcome;

pub struct AmendView {
	pub output: CliViewContext,
	pub progress_open: bool,
}

impl View<AmendOutcome, AmendOutcome, Progress> for AmendView {
	type Completion = ();
	type Error = io::Error;

	async fn confirm(
		&mut self,
		request: trine::invocation::Confirmation,
		cancellation: trine::Cancellation,
	) -> io::Result<Option<bool>> {
		let answer = self.output.confirm(&request, &cancellation)?;
		if answer.is_some() {
			self.progress_open = false;
		}
		Ok(answer)
	}

	async fn progress(&mut self, progress: Progress) -> io::Result<()> {
		let mut terminal = Terminal::for_output(&self.output.stderr);
		terminal.progress(&progress);
		self.output.stderr.write_all(terminal.output.as_bytes())?;
		self.progress_open = true;
		Ok(())
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

		if self.progress_open {
			self.output.stderr.write_all(b"\n")?;
		}

		let mut terminal = Terminal::for_output(&self.output.stdout);
		if succeeded && unchanged {
			terminal.heading(&format!("No changes needed: {}", text(&outcome.task)));
		} else {
			terminal.heading(&format!(
				"Amendment {}: {}",
				outcome.operation.outcome.as_str(),
				text(&outcome.task)
			));
		}

		if succeeded {
			terminal.participant_changes(&outcome.changes, &outcome.participants);
		} else if !unchanged {
			let changed = outcome
				.changes
				.iter()
				.filter(|change| change.kind != "unchanged")
				.map(|change| format!("{} ({})", text(&change.participant), change.kind))
				.collect::<Vec<_>>()
				.join(", ");
			separate(&mut terminal);
			terminal.hint_at(
				MessageScope::Section,
				&format!("Changes retained: {changed}"),
			);
		}

		if let Some(problem) = &outcome.registration_problem {
			separate(&mut terminal);
			terminal.problem_at(MessageScope::Section, problem);
		}
		if let Some(problem) = &outcome.problem
			&& problem.code != "cancelled"
		{
			separate(&mut terminal);
			terminal.problem_at(MessageScope::Section, problem);
		}

		self.output.stdout.write_all(terminal.output.as_bytes())?;
		Ok(())
	}
}
