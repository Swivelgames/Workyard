use super::super::CreateOutcome;
use crate::{
	contracts::Progress,
	presentation::cli::render::{Terminal, Tone},
};
use std::io;
use trine::{application::CliViewContext, view::View};
use workyard_core::OperationOutcome;

pub struct CreateView {
	pub output: CliViewContext,
}

impl View<CreateOutcome, CreateOutcome, Progress> for CreateView {
	type Completion = ();
	type Error = io::Error;

	async fn progress(&mut self, progress: Progress) -> io::Result<()> {
		let mut terminal = Terminal::for_output(&self.output.stderr);
		terminal.progress(&progress);
		self.output.stderr.write_all(terminal.output.as_bytes())
	}

	async fn finish(&mut self, result: &Result<CreateOutcome, CreateOutcome>) -> io::Result<()> {
		let outcome = match result {
			Ok(outcome) | Err(outcome) => outcome,
		};
		let mut terminal = Terminal::for_output(&self.output.stdout);
		let succeeded = outcome.operation.outcome == OperationOutcome::Succeeded;
		if !outcome.pull_failures.is_empty() && outcome.problem.is_none() {
			terminal.completed_with_pull_failures("Creation", &outcome.task);
		} else {
			terminal.operation_result("Creation", outcome.operation.outcome);
		}
		terminal.workspace(
			outcome.tasks_root.as_ref().map(|path| path.as_path()),
			outcome.state_dir.as_ref().map(|path| path.as_path()),
		);
		let condition = if outcome.lifecycle.is_none() {
			"not inspected"
		} else if outcome
			.participants
			.iter()
			.all(|participant| participant.phase == "completed")
		{
			"active"
		} else {
			"needs attention"
		};
		terminal.task(
			&outcome.task,
			condition,
			if condition == "active" {
				Tone::Good
			} else {
				Tone::Attention
			},
		);
		terminal.operation_participants(
			&outcome.participants,
			outcome.tasks_root.as_ref().map(|path| path.as_path()),
			&outcome.task,
			if succeeded {
				"created"
			} else {
				"creation recorded"
			},
		);
		terminal.pull_failures(&outcome.pull_failures);
		if let Some(problem) = &outcome.problem {
			terminal.problem(problem);
			if problem.remedy.trim().is_empty() {
				terminal.status_guidance(
					&outcome.task,
					outcome.state_dir.as_ref().map(|path| path.as_path()),
				);
			}
		}
		self.output.stdout.write_all(terminal.output.as_bytes())?;
		Ok(())
	}
}
