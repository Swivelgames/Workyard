use super::{Style, Terminal, Tone};
use crate::presentation::cli::render::ParticipantRow;

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
