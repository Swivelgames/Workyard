//! Project agent input into the shared amendment while binding file discard to a disclosure.
use super::AmendInput;
use crate::{contracts::RepositoryInput, execution::removal::RemovalRequest};
use schemars::JsonSchema;
use serde::Deserialize;
use trine::NativePath;

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(super) struct McpAmendInput {
	pub task: String,
	/// Tracked project references or explicit repository paths to include.
	#[serde(default)]
	pub projects: Vec<NativePath>,
	/// Supply a name for an existing repository to rename its participant.
	#[serde(default)]
	pub repositories: Vec<RepositoryInput>,
	/// Names of current participants to remove from this task.
	#[serde(default)]
	pub remove: Vec<String>,
	/// Approve only the files identified by a previously reviewed decision token.
	pub approve_discard: Option<String>,
	/// Workspace directory containing .workyard. Defaults to ancestor discovery.
	pub workspace: Option<NativePath>,
	/// Reuse existing task branches for new participants.
	#[serde(default)]
	pub checkout: bool,
	/// Skip fetching repositories for new participants.
	#[serde(default)]
	pub no_fetch: bool,
	/// Default remote for the selected repositories.
	pub remote: Option<String>,
	/// Starting reference for new branches.
	pub from: Option<String>,
}

impl From<McpAmendInput> for RemovalRequest<AmendInput> {
	fn from(input: McpAmendInput) -> Self {
		let action = AmendInput {
			task: input.task,
			projects: input.projects,
			repositories: input.repositories,
			workspace: input.workspace,
			checkout: input.checkout,
			no_fetch: input.no_fetch,
			remote: input.remote,
			from: input.from,

			remove: input.remove,
			yes: false,
			force: false,
		};
		Self::review_discard(action, input.approve_discard)
	}
}
