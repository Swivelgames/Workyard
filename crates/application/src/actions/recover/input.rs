use schemars::JsonSchema;
use serde::Deserialize;
use trine::NativePath;

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RecoverInput {
	pub task: String,
	/// Explicit workspace directory containing .workyard.
	pub workspace: Option<NativePath>,
	#[serde(default)]
	pub accept_pull: Vec<String>,
	#[serde(default)]
	pub undo: bool,
	#[serde(default)]
	pub yes: bool,
	#[serde(default)]
	pub force: bool,
}
