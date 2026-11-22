use schemars::JsonSchema;
use serde::Deserialize;
use trine::NativePath;

/// Optional task and workspace selectors for live Git inspection.
#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct StatusInput {
	/// Task selected for this operation.
	pub task: Option<String>,
	/// Explicit workspace directory containing .workyard.
	pub workspace: Option<NativePath>,
}
