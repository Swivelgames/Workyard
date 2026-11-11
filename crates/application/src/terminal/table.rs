use super::{Terminal, Tone, text};
use std::fmt::Write;
use unicode_width::UnicodeWidthStr;

pub(crate) struct ParticipantRow {
	pub name: String,
	pub state: String,
	pub tone: Tone,
}

impl Terminal {
	pub fn participants(&mut self, rows: &[ParticipantRow]) {
		if rows.is_empty() {
			return;
		}
		let rows: Vec<_> = rows
			.iter()
			.map(|row| ParticipantRow {
				name: text(&row.name),
				state: text(&row.state),
				tone: row.tone,
			})
			.collect();
		let width = rows
			.iter()
			.map(|row| row.name.width())
			.max()
			.unwrap_or(0)
			.max("Repository".len());
		writeln!(
			self.output,
			"        {}{}  {}",
			self.style.paint("Repository", Tone::Header),
			" ".repeat(width - "Repository".len()),
			self.style.paint("State", Tone::Header)
		)
		.expect("string output");
		for row in rows {
			writeln!(
				self.output,
				"        {}{}  {}",
				self.style.paint(&row.name, Tone::Section),
				" ".repeat(width - row.name.width()),
				self.style.paint(&row.state, row.tone)
			)
			.expect("string output");
		}
	}
}
