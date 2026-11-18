use super::{Terminal, Tone, text};
use std::fmt::Write;
use unicode_width::UnicodeWidthStr;

pub(crate) struct ParticipantRow {
	pub name: String,
	pub state: String,
	pub tone: Tone,
}

/// Locations and backing storage are explicit columns, separate from membership.
pub(crate) struct LocatedParticipantRow {
	pub participant: ParticipantRow,
	pub location: String,
	pub backing: String,
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

impl Terminal {
	pub fn located_participants(&mut self, rows: &[LocatedParticipantRow]) {
		if rows.is_empty() {
			return;
		}
		let columns = ["Participant", "Location", "Backing repo", "State"];
		let values: Vec<_> = rows
			.iter()
			.map(|row| {
				[
					text(&row.participant.name),
					text(&row.location),
					text(&row.backing),
					text(&row.participant.state),
				]
			})
			.collect();
		let mut widths = columns.map(str::len);
		for row in &values {
			for (index, value) in row.iter().enumerate() {
				widths[index] = widths[index].max(value.width());
			}
		}
		for (index, value) in columns.iter().enumerate() {
			if index == 0 {
				self.output.push_str("        ");
			}
			self.output.push_str(&self.style.paint(value, Tone::Header));
			if index < 3 {
				self.output
					.push_str(&" ".repeat(widths[index] - value.width() + 2));
			}
		}
		self.output.push('\n');
		for (row, facts) in values.iter().zip(rows) {
			self.output.push_str("        ");
			for (index, value) in row.iter().enumerate() {
				self.output.push_str(&self.style.paint(
					value,
					if index == 3 {
						facts.participant.tone
					} else {
						Tone::Neutral
					},
				));
				if index < 3 {
					self.output
						.push_str(&" ".repeat(widths[index] - value.width() + 2));
				}
			}
			self.output.push('\n');
		}
	}
}
