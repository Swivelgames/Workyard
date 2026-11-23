use serde::Serialize;
use trine::NativePath;
use workyard_core::{OperationKind, Problem, Task};

pub use crate::contracts::{OperationReport, ParticipantOutcome};
use crate::contracts::{ProblemOutcome, removal};

/// Result of reconciling a single task and its participants.
#[derive(Serialize)]
pub struct RecoverOutcome {
	#[serde(skip)]
	pub(crate) explicit_workspace: Option<NativePath>,
	/// Failure to record a project root after participant work succeeded.
	#[serde(skip_serializing_if = "Option::is_none")]
	pub registration_problem: Option<ProblemOutcome>,
	/// Verified no-op; task/evidence history was preserved.
	pub nothing_to_recover: bool,
	/// User-facing health classification for the observed task.
	#[serde(skip_serializing_if = "Option::is_none")]
	pub condition: Option<crate::contracts::Condition>,
	/// Whether the invocation entered the mutation phase.
	#[serde(skip_serializing_if = "Option::is_none")]
	pub mutation_started: Option<bool>,
	/// Removal or restoration evidence associated with this task.
	#[serde(flatten)]
	pub removal: removal::RemovalReport,
	/// Invocation history tracked separately from task lifecycle.
	pub operation: OperationReport,
	/// None when this invocation could not establish task membership.
	pub lifecycle: Option<&'static str>,
	/// Task selected for this operation.
	pub task: String,
	/// Directory containing the task workspaces.
	pub tasks_root: Option<NativePath>,
	/// Directory holding authoritative Workyard state.
	pub state_dir: Option<NativePath>,
	/// Location of the task record when available.
	pub record: Option<NativePath>,
	/// Members associated with this task or operation.
	pub participants: Vec<ParticipantOutcome>,
	/// Changes reported for individual participants.
	#[serde(skip_serializing_if = "Vec::is_empty")]
	pub changes: Vec<crate::contracts::ParticipantChange>,
	/// Diagnostic describing a failure or an unresolved condition.
	#[serde(skip_serializing_if = "Option::is_none")]
	pub problem: Option<ProblemOutcome>,
}

impl RecoverOutcome {
	/// Present a removal or restoration result with its participant evidence.
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

	/// Build an invocation outcome for a request rejected before effects.
	pub fn rejected(task: String, problem: Problem) -> Self {
		Self {
			changes: Vec::new(),
			registration_problem: None,
			nothing_to_recover: false,
			condition: None,
			mutation_started: None,
			removal: Default::default(),
			operation: OperationReport::rejected(OperationKind::Recover, &problem),
			explicit_workspace: None,
			lifecycle: None,
			task,
			tasks_root: None,
			state_dir: None,
			record: None,
			participants: Vec::new(),
			problem: Some(problem.into()),
		}
	}

	/// Present the recorded task and its participants.
	pub fn from_task(task: Task, record: std::path::PathBuf) -> Self {
		let participants = removal::participants(task.participants, task.removal.as_ref());

		Self {
			changes: Vec::new(),
			registration_problem: None,
			nothing_to_recover: false,
			condition: None,
			mutation_started: None,
			removal: Default::default(),
			operation: task.operation.into(),
			explicit_workspace: None,
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

/// Result of either named task recovery or workspace-wide reconciliation.
#[derive(Serialize)]
#[serde(untagged)]
pub enum RecoveryOutcome {
	/// Result of reconciling one named task.
	Task(Box<RecoverOutcome>),
	/// Result of reconciling workspace placement.
	Workspace(super::workspace::WorkspaceRecovery),
}
