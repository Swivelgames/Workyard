use serde::Serialize;
use trine::{Cancellation, NativePath};
use workyard_core::{OperationKind, Problem, Task};

use crate::contracts::ProblemOutcome;
pub use crate::contracts::{OperationReport, ParticipantOutcome};

pub(crate) struct ExistingTaskPreview {
	pub workspace: Option<NativePath>,
	pub cancellation: Cancellation,
}

/// Task placement, participant progress and the result of the creation attempt.
#[derive(Serialize)]
pub struct CreateOutcome {
	#[serde(skip)]
	pub(crate) explicit_workspace: Option<NativePath>,
	#[serde(skip)]
	pub(crate) checkout_hint: Option<String>,
	#[serde(skip)]
	pub(crate) starting_branch_example: Option<String>,
	#[serde(skip)]
	pub(crate) existing_task: Option<ExistingTaskPreview>,
	/// Failure to record a project root after participant work succeeded.
	#[serde(skip_serializing_if = "Option::is_none")]
	pub registration_problem: Option<ProblemOutcome>,
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
	/// Diagnostic describing a failure or an unresolved condition.
	#[serde(skip_serializing_if = "Option::is_none")]
	pub problem: Option<ProblemOutcome>,
}

impl CreateOutcome {
	/// Build an invocation outcome for a request rejected before effects.
	pub fn rejected(task: String, problem: Problem) -> Self {
		Self {
			checkout_hint: None,
			starting_branch_example: None,
			existing_task: None,
			registration_problem: None,
			operation: OperationReport::rejected(OperationKind::Create, &problem),
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
		Self {
			checkout_hint: None,
			starting_branch_example: None,
			existing_task: None,
			registration_problem: None,
			operation: task.operation.into(),
			explicit_workspace: None,
			lifecycle: Some(task.lifecycle.as_str()),
			task: task.name,
			tasks_root: Some(NativePath::new(task.root).expect("validated root path")),
			state_dir: Some(NativePath::new(task.state_dir).expect("validated state path")),
			record: Some(NativePath::new(record).expect("validated filesystem path")),
			participants: task.participants.into_iter().map(Into::into).collect(),
			problem: task.problem.map(Into::into),
		}
	}
}
