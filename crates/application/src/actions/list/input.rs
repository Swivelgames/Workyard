use schemars::JsonSchema;
use serde::Deserialize;
use trine::NativePath;

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ListInput {
	/// Explicit workspace directory containing .workyard.
	pub workspace: Option<NativePath>,
}
