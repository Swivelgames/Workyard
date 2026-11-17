//! Optional update failures are invocation facts, not persistent task conditions.
use super::{MessageScope, Terminal};
use crate::contracts::PullFailure;

impl Terminal {
	pub fn completed_with_pull_failures(&mut self, label: &str, task: &str) {
		self.heading(&format!("{label} completed with pull failures: {task}"));
	}

	pub fn pull_failures(&mut self, failures: &[PullFailure]) {
		for failure in failures {
			self.hint_at(
				MessageScope::Workspace,
				&format!("Pull failed for {}:", failure.participant),
			);
			self.problem_at(MessageScope::Workspace, &failure.problem);
		}
	}
}
