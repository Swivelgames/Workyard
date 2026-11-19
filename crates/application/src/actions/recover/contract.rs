use serde::Serialize;
use trine::NativePath;
use workyard_core::{OperationKind, Problem, Task};

pub use crate::contracts::{OperationReport, ParticipantOutcome};
use crate::contracts::{ProblemOutcome, removal};

#[derive(Serialize)]
pub struct RecoverOutcome {
	#[serde(skip_serializing_if = "Option::is_none")]
	pub registration_problem: Option<ProblemOutcome>,
	#[serde(skip_serializing_if = "Vec::is_empty")]
	pub pull_failures: Vec<crate::contracts::PullFailure>,
	/// Verified no-op; task/evidence history was preserved.
	pub nothing_to_recover: bool,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub condition: Option<crate::contracts::Condition>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub mutation_started: Option<bool>,
	#[serde(flatten)]
	pub removal: removal::RemovalReport,
	pub operation: OperationReport,
	/// None when this invocation could not establish task membership.
	pub lifecycle: Option<&'static str>,
	pub task: String,
	pub tasks_root: Option<NativePath>,
	pub state_dir: Option<NativePath>,
	pub record: Option<NativePath>,
	pub participants: Vec<ParticipantOutcome>,
	#[serde(skip_serializing_if = "Vec::is_empty")]
	pub changes: Vec<crate::contracts::ParticipantChange>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub problem: Option<ProblemOutcome>,
}

impl RecoverOutcome {
	pub fn from_removal(result: workyard_core::RemovalResult) -> Self {
		let condition = removal::condition(&result);
		let nothing_to_recover = result.nothing_to_recover;
		let mut outcome = Self::from_task(result.task, result.record);
		outcome.nothing_to_recover = nothing_to_recover;
		outcome.operation = result.operation.into();
		outcome.problem = result.problem.map(Into::into);
		outcome.condition = Some(condition);
		outcome.mutation_started = Some(result.mutation_started);

		if !result.mutation_started && result.inspected {
			removal::mark_present_after_inspection(&mut outcome.participants);
		}

		outcome
	}

	pub fn rejected(task: String, problem: Problem) -> Self {
		Self {
			changes: Vec::new(),
			registration_problem: None,
			pull_failures: Vec::new(),
			nothing_to_recover: false,
			condition: None,
			mutation_started: None,
			removal: Default::default(),
			operation: OperationReport::rejected(OperationKind::Recover, &problem),
			lifecycle: None,
			task,
			tasks_root: None,
			state_dir: None,
			record: None,
			participants: Vec::new(),
			problem: Some(problem.into()),
		}
	}

	pub fn from_task(task: Task, record: std::path::PathBuf) -> Self {
		let participants = removal::participants(task.participants, task.removal.as_ref());

		Self {
			changes: Vec::new(),
			registration_problem: None,
			pull_failures: Vec::new(),
			nothing_to_recover: false,
			condition: None,
			mutation_started: None,
			removal: Default::default(),
			operation: task.operation.into(),
			lifecycle: Some(task.lifecycle.as_str()),
			task: task.name,
			tasks_root: Some(NativePath::new(task.root).expect("validated root path")),
			state_dir: Some(NativePath::new(task.state_dir).expect("validated state path")),
			record: Some(NativePath::new(record).expect("validated filesystem path")),
			participants,
			problem: task.problem.map(Into::into),
		}
	}
}

#[derive(Serialize)]
#[serde(untagged)]
pub enum RecoveryOutcome {
	Task(Box<RecoverOutcome>),
	Workspace(super::workspace::WorkspaceRecovery),
}
