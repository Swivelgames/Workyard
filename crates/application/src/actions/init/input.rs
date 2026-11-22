use schemars::JsonSchema;
use serde::Deserialize;
use trine::NativePath;

/// Workspace directory and relative location for task worktrees.
#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct InitInput {
	/// Workspace directory (defaults to the current directory).
	pub directory: Option<NativePath>,
	/// Tasks directory relative to the workspace (defaults to tasks).
	pub tasks: Option<NativePath>,
}
