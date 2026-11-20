use schemars::JsonSchema;
use serde::Deserialize;
use trine::NativePath;

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
	#[serde(default)]
	pub undo: bool,
	#[serde(default)]
	pub yes: bool,
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
