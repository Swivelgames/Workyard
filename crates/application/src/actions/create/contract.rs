use serde::Serialize;
use trine::NativePath;
use workyard_core::{Operation, OperationKind, OperationOutcome, Problem, Task};

use crate::contracts::ProblemOutcome;
pub use crate::contracts::{OperationReport, ParticipantOutcome};

#[derive(Serialize)]
pub struct CreateOutcome {
	#[serde(skip_serializing_if = "Vec::is_empty")]
	pub pull_failures: Vec<crate::contracts::PullFailure>,
	pub operation: OperationReport,
	/// None when this invocation could not establish task membership.
	pub lifecycle: Option<&'static str>,
	pub task: String,
	pub tasks_root: Option<NativePath>,
	pub state_dir: Option<NativePath>,
	pub record: Option<NativePath>,
	pub participants: Vec<ParticipantOutcome>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub problem: Option<ProblemOutcome>,
}

impl CreateOutcome {
	pub fn rejected(task: String, problem: Problem) -> Self {
		let outcome = if problem.code == workyard_core::ProblemCode::Cancelled {
			OperationOutcome::Cancelled
		} else {
			OperationOutcome::Failed
		};
		Self {
			pull_failures: Vec::new(),
			operation: Operation {
				kind: Some(OperationKind::Create),
				continues: None,
				outcome,
			}
			.into(),
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
		Self {
			pull_failures: Vec::new(),
			operation: task.operation.into(),
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
