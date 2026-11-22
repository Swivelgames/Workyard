use schemars::JsonSchema;
use serde::Deserialize;
use trine::NativePath;

/// Repository selection and its per-participant overrides.
#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RepositoryInput {
	/// Path to the backing repository selected by the caller.
	pub path: NativePath,
	/// Optional override for the participant’s directory name inside the task.
	pub name: Option<String>,
	/// Remote selected for branch resolution or fetch.
	pub remote: Option<String>,
	/// Starting revision for a newly created branch.
	pub from: Option<String>,
}
