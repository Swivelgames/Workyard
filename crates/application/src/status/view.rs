use super::StatusOutcome;
use crate::common::Progress;
use std::io;
use trine::{application::CliViewContext, view::View};

pub struct StatusView {
	pub output: CliViewContext,
}

impl View<StatusOutcome, StatusOutcome, Progress> for StatusView {
	type Completion = ();
	type Error = io::Error;

	async fn progress(&mut self, _: Progress) -> io::Result<()> {
		Ok(())
	}

	async fn finish(&mut self, result: &Result<StatusOutcome, StatusOutcome>) -> io::Result<()> {
		let outcome = match result {
			Ok(outcome) | Err(outcome) => &outcome.0,
		};
		let mut terminal = crate::terminal::Terminal::for_output(&self.output.stdout);
		crate::observation::render::status(&mut terminal, outcome);
		self.output.stdout.write_all(terminal.output.as_bytes())?;

		Ok(())
	}
}
