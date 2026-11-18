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
	/// Reuse an existing target branch when available.
	#[serde(default)]
	pub checkout: bool,
	/// Skip fetching from remotes.
	#[serde(default)]
	pub no_fetch: bool,
	/// Run git pull in each new worktree.
	#[serde(default)]
	pub pull: bool,
	/// Default remote for the supplied repositories.
	pub remote: Option<String>,
	/// Starting reference for new branches.
	pub from: Option<String>,
}
