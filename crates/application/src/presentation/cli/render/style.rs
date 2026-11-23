pub(crate) use crate::guidance::text::escaped as text;

#[derive(Clone, Copy)]
pub(crate) enum Tone {
	Identity,
	Section,
	Header,
	Good,
	Attention,
	Neutral,
	Muted,
	Hint,
	SuccessTitle,
	Command,
}

#[derive(Clone, Copy)]
pub(crate) struct Style {
	color: bool,
}

impl Style {
	pub fn new(terminal: bool, no_color: bool, dumb: bool) -> Self {
		Self {
			color: terminal && !no_color && !dumb,
		}
	}

	pub fn paint(self, value: &str, tone: Tone) -> String {
		if !self.color {
			return value.into();
		}
		let code = match tone {
			Tone::Identity => "1;36",
			Tone::Section => "1;34",
			Tone::Header => "4",
			Tone::SuccessTitle => "1;3",
			Tone::Command => "95",
			Tone::Good => "32",
			Tone::Attention => "31",
			Tone::Neutral | Tone::Hint => "33",
			Tone::Muted => "2",
		};
		format!("\x1b[{code}m{value}\x1b[0m")
	}
}
