//! Progress presentation policy, separate from the terminal buffer.
use super::{Terminal, Tone, text};
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
		if event.action == "recover" || (event.action == "remove" && event.phase == "removed") {
			let phase = match event.phase.as_str() {
				"completed" => "recovery step finished",
				"worktree_created" => "worktree created",
				phase => phase,
			};
			writeln!(
				self.output,
				"    {}: {}",
				text(&event.participant),
				text(phase)
			)
			.unwrap();
			return;
		}
		writeln!(
			self.output,
			"{} · {}: {}",
			self.style.paint(&text(event.action), Tone::Section),
			self.style.paint(&text(&event.participant), Tone::Identity),
			text(match event.phase.as_str() {
				"worktree_created" => "worktree created",
				"not_started" => "not started",
				"completed" => "finished",
				phase => phase,
			})
		)
		.expect("string output");
	}
}
