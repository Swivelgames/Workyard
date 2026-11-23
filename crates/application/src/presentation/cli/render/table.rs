use super::{Terminal, Tone, text};
use std::fmt::Write;
use unicode_width::UnicodeWidthStr;

pub(crate) struct ParticipantRow {
	pub name: String,
	pub state: String,
	pub tone: Tone,
}

/// A task participant and its backing repository.
pub(crate) struct StatusParticipantRow {
	pub participant: ParticipantRow,
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
			"      {}{}  {}",
			self.style.paint("Repository", Tone::Header),
			" ".repeat(width - "Repository".len()),
			self.style.paint("State", Tone::Header)
		)
		.expect("string output");

		for row in rows {
			writeln!(
				self.output,
				"      {}{}  {}",
				self.style.paint(&row.name, Tone::Section),
				" ".repeat(width - row.name.width()),
				self.style.paint(&row.state, row.tone)
			)
			.expect("string output");
		}
	}
}

impl Terminal {
	pub fn status_participants(&mut self, rows: &[StatusParticipantRow]) {
		if rows.is_empty() {
			return;
		}

		let columns = ["Name", "Repo", "State"];
		let values: Vec<_> = rows
			.iter()
			.map(|row| {
				[
					text(&row.participant.name),
					text(&row.backing),
					text(&row.participant.state),
				]
			})
			.collect();

		let mut widths = columns.map(str::len);
		for row in &values {
			for (width, value) in widths.iter_mut().zip(row) {
				*width = (*width).max(value.width());
			}
		}

		self.output.push('\n');
		self.status_row(&columns, &widths, [Tone::Header; 3]);
		for (row, facts) in values.iter().zip(rows) {
			self.status_row(
				&row.each_ref().map(String::as_str),
				&widths,
				[Tone::Section, Tone::Identity, facts.participant.tone],
			);
		}
	}

	fn status_row(&mut self, values: &[&str; 3], widths: &[usize; 3], tones: [Tone; 3]) {
		self.output.push_str("      ");
		for (index, value) in values.iter().enumerate() {
			self.output.push_str(&self.style.paint(value, tones[index]));
			if index + 1 < values.len() {
				self.output
					.push_str(&" ".repeat(widths[index] - value.width() + 2));
			}
		}
		self.output.push('\n');
	}
}
