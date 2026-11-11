use super::CreateOutcome;
use crate::{
	common::Progress,
	terminal::{Terminal, Tone},
};
use std::io;
use trine::{
	application::{CliCompletion, CliViewContext},
	view::View,
};
use workyard_core::OperationOutcome;

pub struct CreateView {
	pub output: CliViewContext,
}

impl View<CreateOutcome, CreateOutcome, Progress> for CreateView {
	type Completion = CliCompletion;
	type Error = io::Error;

	async fn progress(&mut self, progress: Progress) -> io::Result<()> {
		let mut terminal = Terminal::for_output(&self.output.stderr);
		terminal.progress(&progress);
		self.output.stderr.write_all(terminal.output.as_bytes())
	}

	async fn finish(
		&mut self,
		result: &Result<CreateOutcome, CreateOutcome>,
	) -> io::Result<CliCompletion> {
		let outcome = match result {
			Ok(outcome) | Err(outcome) => outcome,
		};
		let mut terminal = Terminal::for_output(&self.output.stdout);
		let succeeded = outcome.operation.outcome == OperationOutcome::Succeeded;
		terminal.operation_result("Creation", outcome.operation.outcome);
		terminal.workspace(
			outcome.tasks_root.as_ref().map(|path| path.as_path()),
			outcome.state_dir.as_ref().map(|path| path.as_path()),
		);
		let condition = if outcome.lifecycle.is_none() {
			"not inspected"
		} else if succeeded {
			"active"
		} else {
			"needs attention"
		};
		terminal.task(
			&outcome.task,
			condition,
			if succeeded {
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
		Ok(CliCompletion::exit(if succeeded { 0 } else { 1 }))
	}
}
