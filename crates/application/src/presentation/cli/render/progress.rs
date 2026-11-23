//! Progress presentation policy, separate from the terminal buffer.
use super::{MessageScope, Terminal};
use crate::contracts::Progress;
use std::fmt::Write;

impl Terminal {
	pub fn progress(&mut self, event: &Progress) {
		if let Some(disclosure) = &event.disclosure {
			self.disclosure(disclosure);
			return;
		}
		if event.phase == "disclosed_changes_will_be_discarded" {
			writeln!(
				self.output,
				"    Removing this task will discard the disclosed files. These changes cannot be restored."
			)
			.unwrap();
			return;
		}
		if event.phase == "discarded_changes_not_restorable" {
			writeln!(self.output, "    Discarded changes cannot be restored.").unwrap();
			return;
		}
		let phase = match (event.action, event.phase.as_str()) {
			("recover", "completed") => "recovery step finished",
			(_, "completed") => "finished",
			(_, "repairing_worktree_link") => "repairing Git worktree link",
			(_, "updating_worktree_location") => "updating saved worktree location",
			(_, phase) => phase,
		};
		let phase = phase.replace('_', " ");
		self.note_at(
			MessageScope::Section,
			&format!("{}: {phase}", event.participant),
		);
	}
}
