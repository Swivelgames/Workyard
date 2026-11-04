use serde::Serialize;
use trine::NativePath;
use workyard_core::{Problem, Task, TaskStatus};

use crate::common::ProblemOutcome;

fn serialize_status<S: serde::Serializer>(
	status: &TaskStatus,
	serializer: S,
) -> Result<S::Ok, S::Error> {
	serializer.serialize_str(status.as_str())
}

#[derive(Serialize)]
pub struct ParticipantOutcome {
	pub name: String,
	pub path: NativePath,
	pub phase: String,
}

#[derive(Serialize)]
pub struct CreateOutcome {
	#[serde(serialize_with = "serialize_status")]
	pub status: TaskStatus,
	pub task: String,
	pub record: Option<NativePath>,
	pub participants: Vec<ParticipantOutcome>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub problem: Option<ProblemOutcome>,
}

impl CreateOutcome {
	pub fn rejected(task: String, problem: Problem) -> Self {
		Self {
			status: if problem.code == workyard_core::ProblemCode::Cancelled {
				TaskStatus::Cancelled
			} else {
				TaskStatus::Failed
			},
			task,
			record: None,
			participants: Vec::new(),
			problem: Some(problem.into()),
		}
	}

	pub fn from_task(task: Task, record: std::path::PathBuf) -> Self {
		Self {
			status: task.status,
			task: task.name,
			record: Some(NativePath::new(record).expect("validated filesystem path")),
			participants: task
				.participants
				.into_iter()
				.map(|participant| ParticipantOutcome {
					name: participant.name,
					path: NativePath::new(participant.destination)
						.expect("validated filesystem path"),
					phase: participant.phase.as_str().into(),
				})
				.collect(),
			problem: task.problem.map(Into::into),
		}
	}
}
