use serde::Serialize;
use trine::NativePath;
use workyard_core::{Operation, OperationKind, OperationOutcome, Problem, Task};

use crate::common::ProblemOutcome;
pub use crate::reporting::{OperationReport, ParticipantOutcome};

#[derive(Serialize)]
pub struct RemoveOutcome {
	#[serde(skip_serializing_if = "Option::is_none")]
	pub condition: Option<crate::observation::Condition>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub mutation_started: Option<bool>,
	#[serde(skip_serializing_if = "Vec::is_empty")]
	pub disclosures: Vec<crate::removal::Disclosure>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub decision: Option<crate::removal::DecisionRequirement>,
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

impl RemoveOutcome {
	pub fn from_removal(result: workyard_core::RemovalResult) -> Self {
		let condition = crate::removal::condition(&result);
		let mut outcome = Self::from_task(result.task, result.record);
		outcome.operation = result.operation.into();
		outcome.problem = result.problem.map(Into::into);
		outcome.condition = Some(condition);
		outcome.mutation_started = Some(result.mutation_started);

		if !result.mutation_started && result.inspected {
			for participant in &mut outcome.participants {
				if participant.phase == "completed" {
					participant.phase = "present".into();
				}
			}
		}

		outcome
	}

	pub fn rejected(task: String, problem: Problem) -> Self {
		let outcome = if problem.code == workyard_core::ProblemCode::Cancelled {
			OperationOutcome::Cancelled
		} else {
			OperationOutcome::Failed
		};
		Self {
			condition: None,
			mutation_started: None,
			disclosures: Vec::new(),
			decision: None,
			operation: Operation {
				kind: Some(OperationKind::Remove),
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
		let mut participants: Vec<ParticipantOutcome> = task
			.participants
			.clone()
			.into_iter()
			.map(Into::into)
			.collect();
		if let Some(removal) = &task.removal {
			for (participant, progress) in participants.iter_mut().zip(&removal.participants) {
				participant.phase = match progress.phase {
					workyard_core::RemovalPhase::Present => "present",
					workyard_core::RemovalPhase::Removed => "removed",
					workyard_core::RemovalPhase::Restoring
					| workyard_core::RemovalPhase::Recreated => "restoration_unconfirmed",
					workyard_core::RemovalPhase::Restored => "restored",
				}
				.into();
			}
		}
		Self {
			condition: None,
			mutation_started: None,
			disclosures: Vec::new(),
			decision: None,
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
