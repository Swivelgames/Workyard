use super::ListOutcome;
use crate::common::Progress;
use std::io;
use trine::{application::CliViewContext, view::View};

pub struct ListView {
	pub output: CliViewContext,
}

impl View<ListOutcome, ListOutcome, Progress> for ListView {
	type Completion = ();
	type Error = io::Error;

	async fn progress(&mut self, _: Progress) -> io::Result<()> {
		Ok(())
	}

	async fn finish(&mut self, result: &Result<ListOutcome, ListOutcome>) -> io::Result<()> {
		let outcome = match result {
			Ok(outcome) | Err(outcome) => &outcome.0,
		};
		let mut terminal = crate::terminal::Terminal::for_output(&self.output.stdout);
		crate::observation::render::list(&mut terminal, outcome);
		self.output.stdout.write_all(terminal.output.as_bytes())?;

		Ok(())
	}
}
