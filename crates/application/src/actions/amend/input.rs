use crate::contracts::RepositoryInput;
use schemars::JsonSchema;
use serde::Deserialize;
use trine::NativePath;

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AmendInput {
	pub task: String,
	/// Tracked project references or explicit repository paths.
	#[serde(default)]
	pub projects: Vec<NativePath>,
	#[serde(default)]
	pub repositories: Vec<RepositoryInput>,
	/// Task participant names to remove. Repeat to remove several in one amendment.
	#[serde(default)]
	pub remove: Vec<String>,
	/// Skip confirmation when removing clean participants.
	#[serde(default)]
	pub yes: bool,
	/// Discard disclosed changes in the selected worktrees and skip confirmation.
	#[serde(default)]
	pub force: bool,
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
