use super::{Terminal, paths, text};
use std::fmt::Write;

impl Terminal {
	pub(super) fn disclosure(&mut self, disclosure: &crate::contracts::removal::Disclosure) {
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
