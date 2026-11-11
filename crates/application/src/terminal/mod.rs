//! Shared presentation components; views choose content and completion policy.
mod operation;
mod paths;
mod style;
mod table;

use crate::common::{ProblemOutcome, Progress};
use std::{fmt::Write, path::Path};
pub(crate) use style::{Style, Tone, text};
pub(crate) use table::ParticipantRow;
use trine::application::Output;

#[derive(Clone, Copy)]
pub(crate) enum MessageScope {
	Workspace,
	Task,
}

impl MessageScope {
	fn indent(self) -> &'static str {
		match self {
			Self::Workspace => "",
			Self::Task => "        ",
		}
	}
}

pub(crate) struct Terminal {
	pub output: String,
	pub style: Style,
	pub paths: paths::Paths,
}

impl Terminal {
	/// Policy is captured at the view boundary, independently for each stream.
	pub fn for_output(output: &Output) -> Self {
		let no_color = std::env::var_os("NO_COLOR").is_some_and(|value| !value.is_empty());
		let dumb = std::env::var_os("TERM").is_some_and(|value| value == "dumb");
		Self::new(Style::new(output.is_terminal(), no_color, dumb))
	}

	pub fn new(style: Style) -> Self {
		Self {
			output: String::new(),
			style,
			paths: paths::Paths::discover(),
		}
	}

	pub fn heading(&mut self, title: &str) {
		writeln!(
			self.output,
			"{}",
			self.style.paint(&text(title), Tone::Section)
		)
		.expect("string output");
	}

	pub fn workspace(&mut self, root: Option<&Path>, state: Option<&Path>) {
		if let Some(root) = root {
			let root_label = self.paths.root(root);
			writeln!(
				self.output,
				"  Root:  {}",
				self.style.paint(&root_label, Tone::Identity)
			)
			.expect("string output");
			if let Some(state) = state {
				writeln!(
					self.output,
					"  State: {}",
					self.style
						.paint(&self.paths.within(root, state), Tone::Identity)
				)
				.expect("string output");
			}
			self.output.push('\n');
		}
	}

	pub fn task(&mut self, name: &str, condition: &str, tone: Tone) {
		writeln!(
			self.output,
			"    {} [{}]",
			self.style.paint(&text(name), Tone::Identity),
			self.style.paint(&text(condition), tone)
		)
		.expect("string output");
	}

	pub fn note(&mut self, note: &str) {
		self.note_at(MessageScope::Task, note);
	}

	fn note_at(&mut self, scope: MessageScope, note: &str) {
		writeln!(self.output, "{}{}", scope.indent(), text(note)).expect("string output");
	}

	pub fn hint(&mut self, hint: &str) {
		self.hint_at(MessageScope::Task, hint);
	}

	pub fn hint_at(&mut self, scope: MessageScope, hint: &str) {
		writeln!(
			self.output,
			"{}{}",
			scope.indent(),
			self.style.paint(&text(hint), Tone::Hint)
		)
		.expect("string output");
	}

	pub fn problem(&mut self, problem: &ProblemOutcome) {
		self.problem_at(MessageScope::Task, problem);
	}

	pub fn problem_at(&mut self, scope: MessageScope, problem: &ProblemOutcome) {
		self.note_at(
			scope,
			&format!(
				"{}: {}",
				problem.code,
				problem.message.lines().next().unwrap_or("")
			),
		);
		for line in problem.message.lines().skip(1) {
			self.note_at(scope, line);
		}
		for line in problem.remedy.lines() {
			self.hint_at(scope, line);
		}
	}

	pub fn participant_problem(&mut self, participant: &str, problem: &ProblemOutcome) {
		self.note(&format!("{participant}:"));
		self.problem(problem);
	}

	pub fn progress(&mut self, event: &Progress) {
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

#[cfg(test)]
mod tests {
	use super::{ParticipantRow, Style, Terminal, Tone};

	fn render(style: Style) -> String {
		let mut terminal = Terminal::new(style);
		terminal.heading("Workyard Status");
		terminal.task("feat/payments", "healthy", Tone::Good);
		terminal.participants(&[
			ParticipantRow {
				name: "界".into(),
				state: "clean".into(),
				tone: Tone::Good,
			},
			ParticipantRow {
				name: "api\u{1b}[31m\n".into(),
				state: "modified".into(),
				tone: Tone::Neutral,
			},
		]);
		terminal.output
	}

	fn without_ansi(value: &str) -> String {
		let mut result = String::new();
		let mut characters = value.chars();
		while let Some(character) = characters.next() {
			if character == '\u{1b}' {
				for next in characters.by_ref() {
					if next == 'm' {
						break;
					}
				}
			} else {
				result.push(character);
			}
		}
		result
	}

	#[test]
	fn styled_and_plain_tables_preserve_facts_and_escape_untrusted_controls() {
		let plain = render(Style::new(false, false, false));
		let styled = render(Style::new(true, false, false));
		assert_eq!(without_ansi(&styled), plain);
		assert!(styled.contains("\x1b[1;36mfeat/payments"));
		assert!(!plain.contains('\u{1b}'));
		assert!(plain.contains("api\\u{1b}[31m\\n"));
		let rows: Vec<_> = plain
			.lines()
			.filter(|line| line.starts_with("        "))
			.collect();
		let widths: Vec<_> = rows
			.iter()
			.map(|line| {
				let state = if line.contains("State") {
					"State"
				} else if line.contains("clean") {
					"clean"
				} else {
					"modified"
				};
				unicode_width::UnicodeWidthStr::width(line.split(state).next().unwrap())
			})
			.collect();
		assert!(widths.windows(2).all(|pair| pair[0] == pair[1]));
	}

	#[test]
	fn no_color_and_dumb_terminal_disable_styling_without_losing_content() {
		let plain = render(Style::new(false, false, false));
		assert_eq!(render(Style::new(true, true, false)), plain);
		assert_eq!(render(Style::new(true, false, true)), plain);
	}
}
