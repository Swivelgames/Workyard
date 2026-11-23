//! Output buffer and basic scoped text operations.
use super::{Style, Tone, paths, text};
use std::fmt::Write;
use trine::application::Output;

#[derive(Clone, Copy)]
pub(crate) enum MessageScope {
	Workspace,
	Section,
	Task,
	Detail,
}

impl MessageScope {
	pub(super) fn indent(self) -> &'static str {
		match self {
			Self::Workspace => "  ",
			Self::Section => "    ",
			Self::Task => "      ",
			Self::Detail => "        ",
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
			"  {}",
			self.style.paint(&text(title), Tone::Section)
		)
		.expect("string output");
	}

	pub fn note(&mut self, note: &str) {
		self.note_at(MessageScope::Task, note);
	}

	pub fn note_at(&mut self, scope: MessageScope, note: &str) {
		writeln!(self.output, "{}{}", scope.indent(), text(note)).expect("string output");
	}

	pub fn hint(&mut self, hint: &str) {
		self.hint_at(MessageScope::Task, hint);
	}

	pub fn hint_at(&mut self, scope: MessageScope, hint: &str) {
		self.advice_at(scope, hint, Some(Tone::Hint));
	}

	pub fn advice_at(&mut self, scope: MessageScope, advice: &str, fallback: Option<Tone>) {
		let advice = text(advice);
		let styled = self.style.command_references(&advice).unwrap_or_else(|| {
			fallback.map_or_else(|| advice.clone(), |tone| self.style.paint(&advice, tone))
		});
		writeln!(self.output, "{}{styled}", scope.indent()).expect("string output");
	}
}

#[cfg(test)]
mod tests;
