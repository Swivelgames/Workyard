use serde::Serialize;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Condition {
	Healthy,
	NeedsAttention,
	Unknown,
}

impl Condition {
	pub fn label(self) -> &'static str {
		match self {
			Self::Healthy => "healthy",
			Self::NeedsAttention => "needs attention",
			Self::Unknown => "unknown",
		}
	}
}
