use schemars::JsonSchema;
use serde::Deserialize;
use trine::NativePath;

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RepositoryInput {
	pub path: NativePath,
	pub name: Option<String>,
	pub remote: Option<String>,
	pub from: Option<String>,
}
