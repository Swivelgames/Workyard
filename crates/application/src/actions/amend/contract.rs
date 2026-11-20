use crate::contracts::{OperationReport, ParticipantChange, ParticipantOutcome, ProblemOutcome};
use serde::Serialize;
use trine::NativePath;
use workyard_core::{AmendResult, OperationKind, Problem};

#[derive(Serialize)]
pub struct AmendOutcome {
	#[serde(flatten)]
	pub removal: crate::contracts::removal::RemovalReport,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub registration_problem: Option<ProblemOutcome>,
	pub operation: OperationReport,
	pub task: String,
	pub lifecycle: Option<&'static str>,
	pub state_dir: Option<NativePath>,
	pub participants: Vec<ParticipantOutcome>,
	pub changes: Vec<ParticipantChange>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub problem: Option<ProblemOutcome>,
}

impl AmendOutcome {
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

	pub(super) fn guide_retirement(&mut self, state: &std::path::Path, explicit_workspace: bool) {
		let Some(problem) = self
			.problem
			.as_mut()
			.filter(|p| p.code == workyard_core::ProblemCode::EmptyTask.as_str())
		else {
			return;
		};
		let scope = explicit_workspace.then_some(state);
		if let Some(command) = crate::guidance::commands::remove(&self.task, scope) {
			problem.remedy = format!("To retire the task, run {command}");
		}
		self.state_dir = NativePath::new(state.to_path_buf()).ok();
	}

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
