use schemars::JsonSchema;
use serde::Deserialize;
use trine::NativePath;

use crate::contracts::RepositoryInput;

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CreateInput {
	pub task: String,
	/// Tracked project references or explicit repository paths.
	#[serde(default)]
	pub projects: Vec<NativePath>,
	#[serde(default)]
	pub repositories: Vec<RepositoryInput>,
	/// Explicit workspace directory containing .workyard.
	pub workspace: Option<NativePath>,
	/// Reuse an existing target branch when available.
	#[serde(default)]
	pub checkout: bool,
	/// Skip fetching from remotes.
	#[serde(default)]
	pub no_fetch: bool,
	/// Default remote for the supplied repositories.
	pub remote: Option<String>,
	/// Starting reference for new branches.
	pub from: Option<String>,
}
