//! Labels and tones shared by task summaries and other bracketed tags.
use super::{Terminal, Tone, text};
use crate::observation::{Condition, TaskReport};
use std::fmt::Write;

#[derive(Clone, Copy)]
pub(crate) enum TaskTag {
	Active,
	Healthy,
	NeedsAttention,
	Unknown,
	NotInspected,
	Retired,
}

impl TaskTag {
	pub fn observed(task: &TaskReport) -> Self {
		if task.lifecycle == Some("retired") {
			return Self::Retired;
		}

		match task.condition {
			Condition::Healthy => Self::Healthy,
			Condition::NeedsAttention => Self::NeedsAttention,
			Condition::Unknown => Self::Unknown,
		}
	}

	pub(super) fn appearance(&self) -> (&'static str, Tone) {
		match self {
			Self::Active => ("active", Tone::Good),
			Self::Healthy => (Condition::Healthy.label(), Tone::Good),
			Self::NeedsAttention => (Condition::NeedsAttention.label(), Tone::Attention),
			Self::Unknown => (Condition::Unknown.label(), Tone::Neutral),
			Self::NotInspected => ("not inspected", Tone::Neutral),
			Self::Retired => ("retired", Tone::Muted),
		}
	}
}

impl Terminal {
	pub fn tag(&self, label: &str, tone: Tone) -> String {
		format!("[{}]", self.style.paint(&text(label), tone))
	}

	pub fn task(&mut self, name: &str, tag: TaskTag, members: &str) {
		let name = self.style.paint(&text(name), Tone::Identity);
		let (label, tone) = tag.appearance();
		let tag = self.tag(label, tone);

		write!(self.output, "    {name} {tag}").expect("string output");
		if !members.is_empty() {
			write!(self.output, "  {}", text(members)).expect("string output");
		}
		self.output.push('\n');
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::presentation::cli::render::Style;

	#[test]
	fn compact_and_detailed_task_rows_use_the_same_tag_colors() {
		for (tag, label, color) in [
			(TaskTag::Active, "active", 32),
			(TaskTag::Healthy, "healthy", 32),
			(TaskTag::NeedsAttention, "needs attention", 31),
			(TaskTag::Unknown, "unknown", 33),
			(TaskTag::NotInspected, "not inspected", 33),
			(TaskTag::Retired, "retired", 2),
		] {
			let mut styled = Terminal::new(Style::new(true, false, false));
			styled.task("feat/task", tag, "api");
			assert_eq!(
				styled.output,
				format!("    \x1b[1;36mfeat/task\x1b[0m [\x1b[{color}m{label}\x1b[0m]  api\n")
			);

			let mut plain = Terminal::new(Style::new(true, true, false));
			plain.task("feat/task", tag, "");
			assert_eq!(plain.output, format!("    feat/task [{label}]\n"));
		}
	}
}
