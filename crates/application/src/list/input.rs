use schemars::JsonSchema;
use serde::Deserialize;
use trine::NativePath;

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ListInput {
	pub tasks_root: Option<NativePath>,
	pub state_dir: Option<NativePath>,
}
