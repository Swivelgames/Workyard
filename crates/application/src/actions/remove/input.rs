use schemars::JsonSchema;
use serde::Deserialize;
use trine::NativePath;

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RemoveInput {
	pub task: String,
	/// Explicit workspace directory containing .workyard.
	pub workspace: Option<NativePath>,
	/// Skip removal confirmation.
	#[serde(default)]
	pub yes: bool,
	/// Allow removal of dirty worktrees and skip confirmation.
	#[serde(default)]
	pub force: bool,
}
