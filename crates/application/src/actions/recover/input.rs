use schemars::JsonSchema;
use serde::Deserialize;
use trine::NativePath;

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RecoverInput {
	/// Resume a named task operation. Omit for workspace location recovery.
	pub task: Option<String>,
	/// Use the verified worktrees in this workspace when multiple copies exist.
	#[serde(default)]
	pub activate_here: bool,
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
