use std::io;
use trine::{
	application::{CliCompletion, CliViewContext},
	view::View,
};
use workyard_core::TaskStatus;

use crate::common::Progress;

use super::CreateOutcome;

pub struct CreateView {
	pub output: CliViewContext,
}

impl View<CreateOutcome, CreateOutcome, Progress> for CreateView {
	type Completion = CliCompletion;
	type Error = io::Error;

	async fn progress(&mut self, progress: Progress) -> io::Result<()> {
		self.output
			.stderr
			.write_all(format!("{}: {}\n", progress.participant, progress.phase).as_bytes())
	}

	async fn finish(
		&mut self,
		result: &Result<CreateOutcome, CreateOutcome>,
	) -> io::Result<CliCompletion> {
		let outcome = match result {
			Ok(outcome) | Err(outcome) => outcome,
		};
		let mut summary = format!("Task {}: {}\n", outcome.task, outcome.status.as_str());
		for participant in &outcome.participants {
			summary.push_str(&format!(
				"  {}: {} ({:?})\n",
				participant.name,
				participant.phase,
				participant.path.as_path()
			));
		}

		if let Some(problem) = &outcome.problem {
			summary.push_str(&format!(
				"{}: {}\n{}\n",
				problem.code, problem.message, problem.remedy
			));
		}

		if let Some(record) = &outcome.record {
			summary.push_str(&format!("State record: {:?}\n", record.as_path()));
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
