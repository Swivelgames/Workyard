use crate::contracts::RepositoryInput;
use schemars::JsonSchema;
use serde::Deserialize;
use trine::NativePath;

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AmendInput {
	pub task: String,
	pub repositories: Vec<RepositoryInput>,
	pub tasks_root: Option<NativePath>,
	pub state_dir: Option<NativePath>,
	#[serde(default)]
	pub checkout: bool,
	#[serde(default)]
	pub no_fetch: bool,
	#[serde(default)]
	pub pull: bool,
	pub remote: Option<String>,
	pub from: Option<String>,
}
