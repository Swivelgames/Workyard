//! Output buffer and basic scoped text operations.
use super::{Style, Tone, paths, text};
use crate::contracts::ProblemOutcome;
use std::{fmt::Write, path::Path};
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
		for candidate in &problem.candidates {
			let label = paths::display(candidate.as_path());
			writeln!(self.output, "{}    {label}", scope.indent()).expect("string output");
		}
		for line in problem.remedy.lines() {
			self.hint_at(scope, line);
		}
	}

	pub fn participant_problem(&mut self, participant: &str, problem: &ProblemOutcome) {
		self.note(&format!("{participant}:"));
		self.problem(problem);
	}
}

#[cfg(test)]
mod tests;
