use schemars::JsonSchema;
use serde::Deserialize;
use trine::NativePath;

/// Workspace whose active tasks and recent retirements should be listed.
#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ListInput {
	/// Explicit workspace directory containing .workyard.
	pub workspace: Option<NativePath>,
}
