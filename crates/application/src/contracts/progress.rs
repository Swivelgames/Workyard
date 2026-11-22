use serde::Serialize;

/// Participant operation event delivered to observing views.
#[derive(Serialize)]
pub struct Progress {
	/// Removal facts to present before proceeding.
	#[serde(skip_serializing_if = "Option::is_none")]
	pub disclosure: Option<crate::contracts::removal::Disclosure>,
	/// Action producing the event.
	pub action: &'static str,
	/// Task selected for this operation.
	pub task: String,
	/// Participant to which this observation or change applies.
	pub participant: String,
	/// Step reached by this participant.
	pub phase: String,
}
