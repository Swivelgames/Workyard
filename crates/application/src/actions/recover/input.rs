use schemars::JsonSchema;
use serde::Deserialize;
use trine::NativePath;

/// Select a task or the workspace and specify the authorized recovery direction.
#[derive(Default, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RecoverInput {
	/// Resume a named task operation. Omit for workspace location recovery.
	pub task: Option<String>,
	/// Use the verified worktrees in this workspace when multiple copies exist.
	#[serde(default)]
	pub activate_here: bool,
	/// Explicit workspace directory containing .workyard.
	pub workspace: Option<NativePath>,
	/// Resume in the restoration direction.
	#[serde(default)]
	pub undo: bool,
	/// Accept routine confirmation.
	#[serde(default)]
	pub yes: bool,
	/// Authorize the disclosed discard and accept routine confirmation.
	#[serde(default)]
	pub force: bool,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct NamedParticipantRecovery {
	/// Task whose creation, checkout or participant additions and renames should be continued.
	pub task: String,
	/// Workspace directory containing .workyard. Defaults to ancestor discovery.
	pub workspace: Option<NativePath>,
}

#[derive(Deserialize, JsonSchema)]
#[serde(from = "RecoverInput")]
#[schemars(rename = "RecoverInput")]
pub(crate) enum RecoveryRequest {
	General(RecoverInput),
	Participants(NamedParticipantRecovery),
}

impl From<RecoverInput> for RecoveryRequest {
	fn from(input: RecoverInput) -> Self {
		Self::General(input)
	}
}
