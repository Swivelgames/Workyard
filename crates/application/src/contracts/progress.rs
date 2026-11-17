use serde::Serialize;

#[derive(Serialize)]
pub struct Progress {
	#[serde(skip_serializing_if = "Option::is_none")]
	pub disclosure: Option<crate::contracts::removal::Disclosure>,
	pub action: &'static str,
	pub task: String,
	pub participant: String,
	pub phase: String,
}
