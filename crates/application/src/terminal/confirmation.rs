//! Short questions complement the disclosure already delivered by progress.
use super::{Terminal, paths, text};
use crate::removal::DecisionRequirement;
use std::fmt::Write;

pub(crate) fn confirmation_message(question: &DecisionRequirement) -> String {
	let participant = text(&question.participant);
	match question.kind.as_str() {
		"remove_task" | "discard_task" => format!(
			"Are you sure you want to remove {}?",
			text(question.task.as_deref().unwrap_or("this task"))
		),
		"retained_history" => format!("Remove {participant}?"),
		"discard_files" => format!("Discard changes and remove {participant}?"),
		"lost_changes" => "Recreate worktrees from retained branches?".into(),
		_ => text(&question.message),
	}
}

impl Terminal {
	pub(super) fn disclosure(&mut self, disclosure: &crate::removal::Disclosure) {
		let history = match disclosure.history {
			"unpushed" => "unpushed commits",
			"no_upstream" => "no upstream",
			"detached_protected" => "detached; retained reference",
			"up_to_date" => "branch up to date",
			_ => "history unavailable",
		};
		writeln!(
			self.output,
			"    {}: {history}",
			text(&disclosure.participant)
		)
		.expect("string output");
		for path in &disclosure.dirty_paths {
			writeln!(
				self.output,
				"        Would discard: {}",
				paths::display(path.as_path())
			)
			.unwrap();
		}
	}
}
