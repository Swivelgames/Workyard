use crate::contracts::{OperationReport, ParticipantChange, ParticipantOutcome, ProblemOutcome};
use serde::Serialize;
use trine::NativePath;
use workyard_core::{AmendResult, OperationKind, Problem};

/// Membership changes and any outstanding problems after an amendment.
#[derive(Serialize)]
pub struct AmendOutcome {
	/// Removal or restoration evidence associated with this task.
	#[serde(flatten)]
	pub removal: crate::contracts::removal::RemovalReport,
	/// Failure to record a project root after participant work succeeded.
	#[serde(skip_serializing_if = "Option::is_none")]
	pub registration_problem: Option<ProblemOutcome>,
	/// Invocation history tracked separately from task lifecycle.
	pub operation: OperationReport,
	/// Task selected for this operation.
	pub task: String,
	/// Current membership lifecycle of the task.
	pub lifecycle: Option<&'static str>,
	/// Directory holding authoritative Workyard state.
	pub state_dir: Option<NativePath>,
	/// Members associated with this task or operation.
	pub participants: Vec<ParticipantOutcome>,
	/// Changes reported for individual participants.
	pub changes: Vec<ParticipantChange>,
	/// Diagnostic describing a failure or an unresolved condition.
	#[serde(skip_serializing_if = "Option::is_none")]
	pub problem: Option<ProblemOutcome>,
}

impl AmendOutcome {
	/// Build an invocation outcome for a request rejected before effects.
	pub fn rejected(task: String, problem: Problem) -> Self {
		Self {
			removal: Default::default(),
			registration_problem: None,
			operation: OperationReport::rejected(OperationKind::Amend, &problem),
			task,
			lifecycle: None,
			state_dir: None,
			participants: Vec::new(),
			changes: Vec::new(),
			problem: Some(problem.into()),
		}
	}

	pub(super) fn guide_retirement(
		&mut self,
		state: &std::path::Path,
		explicit_workspace: Option<&std::path::Path>,
	) {
		let Some(problem) = self
			.problem
			.as_mut()
			.filter(|p| p.code == workyard_core::ProblemCode::EmptyTask.as_str())
		else {
			return;
		};
		if let Some(command) = crate::guidance::commands::remove(&self.task, explicit_workspace) {
			problem.remedy = format!("To retire the task, run {command}");
		}
		self.state_dir = NativePath::new(state.to_path_buf()).ok();
	}

	/// Convert the core operation result into its public representation.
	pub fn from_result(result: AmendResult) -> Self {
		Self {
			removal: Default::default(),
			registration_problem: None,
			operation: result.operation.into(),
			task: result.task.name,
			lifecycle: Some(result.task.lifecycle.as_str()),
			state_dir: Some(NativePath::new(result.task.state_dir).expect("validated state path")),
			participants: result
				.task
				.participants
				.into_iter()
				.map(Into::into)
				.collect(),
			changes: result.changes.into_iter().map(Into::into).collect(),
			problem: result.problem.map(Into::into),
		}
	}
}
