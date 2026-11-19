use crate::contracts::ProblemOutcome;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use trine::NativePath;

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(super) struct ListInput {
	/// Explicit workspace directory containing .workyard.
	pub workspace: Option<NativePath>,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(super) struct RootsInput {
	/// Repositories or directories of repositories.
	pub paths: Vec<NativePath>,
	/// Explicit workspace directory containing .workyard.
	pub workspace: Option<NativePath>,
}

#[derive(Serialize)]
pub(super) struct Root {
	pub path: NativePath,
	pub status: &'static str,
	#[serde(skip_serializing_if = "Vec::is_empty")]
	pub issues: Vec<ProblemOutcome>,
}

#[derive(Serialize)]
pub(super) struct Project {
	pub reference: NativePath,
	pub path: NativePath,
}

#[derive(Default, Serialize)]
pub(super) struct Inventory {
	pub roots: Vec<Root>,
	pub projects: Vec<Project>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub guidance: Option<String>,
}

#[derive(Serialize)]
pub(super) struct RootChanges {
	pub changed: Vec<NativePath>,
}
