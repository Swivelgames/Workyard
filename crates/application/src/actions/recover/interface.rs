//! Registered wire input adds omitted-task workspace scope without changing RecoverInput.
use super::RecoverOutcome;
use crate::{actions::relocate::RelocateOutcome, contracts::OperationReport};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use trine::NativePath;

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct Input {
	pub task: Option<String>,
	pub tasks_root: Option<NativePath>,
	pub state_dir: Option<NativePath>,
	#[serde(default)]
	pub accept_pull: Vec<String>,
	#[serde(default)]
	pub undo: bool,
	#[serde(default)]
	pub yes: bool,
	#[serde(default)]
	pub force: bool,
}

#[derive(Serialize)]
#[serde(untagged)]
pub(crate) enum Outcome {
	Task(RecoverOutcome),
	Workspace(RelocateOutcome),
}
impl Outcome {
	pub(super) fn operation(&self) -> &OperationReport {
		match self {
			Self::Task(outcome) => &outcome.operation,
			Self::Workspace(outcome) => &outcome.operation,
		}
	}
}
