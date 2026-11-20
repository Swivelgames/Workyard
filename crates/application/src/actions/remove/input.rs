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

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(super) struct McpRemoveInput {
	pub task: String,
	/// Workspace directory containing .workyard. Defaults to ancestor discovery.
	pub workspace: Option<NativePath>,
	/// Approve only the files identified by a previously reviewed decision token.
	pub approve_discard: Option<String>,
}

impl From<McpRemoveInput> for crate::execution::removal::RemovalRequest<RemoveInput> {
	fn from(input: McpRemoveInput) -> Self {
		let action = RemoveInput {
			task: input.task,
			workspace: input.workspace,
			yes: false,
			force: false,
		};
		Self::review_discard(action, input.approve_discard)
	}
}
