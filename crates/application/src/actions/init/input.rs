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
	/// Tasks directory outside the workspace, expressed relative to it.
	pub detached_tasks: Option<NativePath>,
	/// Limit task participants to discovered projects.
	#[serde(default)]
	pub tracked_only: bool,
	/// Initial repository or directory roots. Defaults to projects/ when omitted.
	#[serde(default)]
	pub track: Vec<NativePath>,
}
