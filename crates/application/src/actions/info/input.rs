use schemars::JsonSchema;
use serde::Deserialize;
use trine::NativePath;

/// Workspace selection for a summary of placement and task counts.
#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct InfoInput {
	/// Explicit workspace directory containing .workyard.
	pub workspace: Option<NativePath>,
}
