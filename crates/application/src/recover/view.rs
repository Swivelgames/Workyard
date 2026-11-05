use std::io;
use trine::{
	application::{CliCompletion, CliViewContext},
	view::View,
};
use workyard_core::TaskStatus;

use crate::{common::Progress, terminal};

use super::RecoverOutcome;

pub struct RecoverView {
	pub output: CliViewContext,
}

impl View<RecoverOutcome, RecoverOutcome, Progress> for RecoverView {
	type Completion = CliCompletion;
	type Error = io::Error;

	async fn progress(&mut self, progress: Progress) -> io::Result<()> {
		self.output
			.stderr
			.write_all(terminal::progress(&progress).as_bytes())
	}

	async fn finish(
		&mut self,
		result: &Result<RecoverOutcome, RecoverOutcome>,
	) -> io::Result<CliCompletion> {
		let outcome = match result {
			Ok(outcome) | Err(outcome) => outcome,
		};
		let mut summary = String::new();
		terminal::task(&mut summary, &outcome.task, outcome.status);
		for participant in &outcome.participants {
			terminal::participant(&mut summary, participant);
		}

		if let Some(problem) = &outcome.problem {
			terminal::problem(&mut summary, problem);
		}

		if let Some(record) = &outcome.record {
			terminal::state_location(&mut summary, record);
		}

		self.output.stdout.write_all(summary.as_bytes())?;
		Ok(CliCompletion::exit(
			if outcome.status == TaskStatus::Completed {
				0
			} else {
				1
			},
		))
	}
}
