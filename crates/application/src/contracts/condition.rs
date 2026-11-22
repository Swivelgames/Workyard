use serde::Serialize;

/// User-facing assessment of current task health.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Condition {
	/// Available observations indicate a healthy task.
	Healthy,
	/// Observations identify an issue requiring attention.
	NeedsAttention,
	/// Available evidence is insufficient to determine health.
	Unknown,
}

impl Condition {
	/// Return the human-readable status label.
	pub fn label(self) -> &'static str {
		match self {
			Self::Healthy => "healthy",
			Self::NeedsAttention => "needs attention",
			Self::Unknown => "unknown",
		}
	}
}
