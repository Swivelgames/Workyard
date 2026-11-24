//! Style known command names in examples and in existing diagnostic guidance.
use super::{MessageScope, Style, Terminal, Tone, text};
use std::fmt::Write;

const WORKYARD_ACTIONS: &[&str] = &[
	"projects list",
	"projects track",
	"projects untrack",
	"amend",
	"checkout",
	"create",
	"info",
	"init",
	"list",
	"recover",
	"remove",
	"status",
];

const GIT_ACTIONS: &[&str] = &[
	"worktree list",
	"worktree prune",
	"worktree repair",
	"status",
];

impl Terminal {
	pub fn command_at(&mut self, scope: MessageScope, command: &str) {
		let command = text(command);
		let indent = scope.indent();
		if let Some((length, styled)) = self.style.command_prefix(&command) {
			writeln!(self.output, "{indent}{styled}{}", &command[length..]).expect("string output");
		} else {
			writeln!(self.output, "{indent}{command}").expect("string output");
		}
	}
}

impl Style {
	/// Input has already been escaped for terminal display.
	pub(super) fn command_references(self, line: &str) -> Option<String> {
		let mut output = String::new();
		let mut written = 0;

		for (start, _) in line.char_indices() {
			if start < written {
				continue;
			}
			if start > 0 && !line[..start].ends_with(before_command) {
				continue;
			}
			let Some((length, styled)) = self.command_prefix(&line[start..]) else {
				continue;
			};

			output.push_str(&line[written..start]);
			output.push_str(&styled);
			written = start + length;
		}

		if written == 0 {
			return None;
		}
		output.push_str(&line[written..]);
		Some(output)
	}

	fn command_prefix(self, line: &str) -> Option<(usize, String)> {
		let (program, action) = command_head(line)?;
		let mut styled = self.paint(
			if program == "workyard" {
				crate::identity::Identity::prefix()
			} else {
				program
			},
			Tone::Hint,
		);
		let mut length = program.len();
		if let Some(action) = action {
			styled.push(' ');
			styled.push_str(&self.paint(action, Tone::Command));
			length += 1 + action.len();
		}
		Some((length, styled))
	}
}

fn command_head(tail: &str) -> Option<(&str, Option<&str>)> {
	let prefix = crate::identity::Identity::prefix();
	let (program, actions) = if tail
		.strip_prefix(prefix)
		.is_some_and(|rest| rest.starts_with(' '))
	{
		(prefix, WORKYARD_ACTIONS)
	} else if tail.starts_with("workyard ") {
		("workyard", WORKYARD_ACTIONS)
	} else if tail.starts_with("git ") {
		("git", GIT_ACTIONS)
	} else {
		return None;
	};

	let rest = &tail[program.len() + 1..];
	for &action in actions {
		if let Some(next) = rest.strip_prefix(action)
			&& (next.is_empty() || next.starts_with(after_command))
		{
			return Some((program, Some(action)));
		}
	}

	// The -C operand can be shell-quoted. Leave it and the following text intact.
	if program == "git" && rest.starts_with("-C ") {
		return Some((program, None));
	}
	None
}

fn before_command(c: char) -> bool {
	c.is_whitespace() || matches!(c, '`' | '\'' | '"' | '(')
}

fn after_command(c: char) -> bool {
	c.is_whitespace()
		|| matches!(
			c,
			'`' | '\'' | '"' | ')' | '.' | ',' | ':' | ';' | '?' | '!'
		)
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn advice_colors_commands_without_changing_copyable_text() {
		let advice = "Try workyard projects track './my repos' or git worktree list.";
		let expected = "Try \x1b[33mworkyard\x1b[0m \x1b[95mprojects track\x1b[0m './my repos' or \x1b[33mgit\x1b[0m \x1b[95mworktree list\x1b[0m.";
		assert_eq!(
			Style::new(true, false, false)
				.command_references(advice)
				.as_deref(),
			Some(expected)
		);
		assert_eq!(
			Style::new(true, true, false)
				.command_references(advice)
				.as_deref(),
			Some(advice)
		);
	}

	#[test]
	fn complete_command_leaves_quoted_arguments_plain() {
		let mut terminal = Terminal::new(Style::new(true, false, false));
		terminal.command_at(
			MessageScope::Section,
			"workyard checkout --repo='workyard status'",
		);
		assert_eq!(
			terminal.output,
			"    \x1b[33mworkyard\x1b[0m \x1b[95mcheckout\x1b[0m --repo='workyard status'\n",
		);
	}

	#[test]
	fn names_inside_paths_and_longer_words_are_left_alone() {
		let style = Style::new(true, false, false);
		assert!(
			style
				.command_references("Inspect /tmp/workyard status-old or workyard status-extra.")
				.is_none()
		);
	}
}
