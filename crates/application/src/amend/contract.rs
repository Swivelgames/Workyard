use crate::{
	common::ProblemOutcome,
	reporting::{OperationReport, ParticipantOutcome},
};
use serde::Serialize;
use trine::NativePath;
use workyard_core::{AmendResult, Operation, OperationKind, OperationOutcome, Problem};

#[derive(Serialize)]
pub struct ParticipantChange {
	pub participant: String,
	pub kind: &'static str,
}

#[derive(Serialize)]
pub struct AmendOutcome {
	#[serde(skip_serializing_if = "Vec::is_empty")]
	pub pull_failures: Vec<crate::reporting::PullFailure>,
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
		let outcome = if problem.code == workyard_core::ProblemCode::Cancelled {
			OperationOutcome::Cancelled
		} else {
			OperationOutcome::Failed
		};
		Self {
			pull_failures: Vec::new(),
			operation: Operation {
				kind: Some(OperationKind::Amend),
				continues: None,
				outcome,
			}
			.into(),
			task,
			lifecycle: None,
			state_dir: None,
			participants: Vec::new(),
			changes: Vec::new(),
			problem: Some(problem.into()),
		}
	}

	pub fn from_result(result: AmendResult) -> Self {
		Self {
			pull_failures: result.pull_failures.into_iter().map(Into::into).collect(),
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
			changes: result
				.changes
				.into_iter()
				.map(|change| ParticipantChange {
					participant: change.participant,
					kind: change.kind.as_str(),
				})
				.collect(),
			problem: result.problem.map(Into::into),
		}
	}
}
