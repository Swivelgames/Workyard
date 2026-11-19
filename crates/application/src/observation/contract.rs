mod participant;
pub use participant::{ParticipantReport, ParticipantState};

use crate::contracts::ProblemOutcome;
use serde::Serialize;
use trine::NativePath;
use workyard_core::{Task, TaskInspection, TaskSnapshot};

#[derive(Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Provenance {
	Recorded,
	Inspected,
}

#[derive(Serialize)]
pub struct SavedTask {
	pub backing_repositories: std::collections::BTreeMap<String, NativePath>,
	pub lifecycle: &'static str,
	pub operation: crate::contracts::OperationReport,
	pub problem: Option<ProblemOutcome>,
	pub participants: Vec<crate::contracts::ParticipantOutcome>,
}

impl From<Task> for SavedTask {
	fn from(task: Task) -> Self {
		Self {
			backing_repositories: task
				.participants
				.iter()
				.map(|participant| {
					(
						participant.name.clone(),
						NativePath::new(&participant.common_dir).expect("recorded backing path"),
					)
				})
				.collect(),
			lifecycle: task.lifecycle.as_str(),
			operation: task.operation.into(),
			problem: task.problem.map(Into::into),
			participants: task.participants.into_iter().map(Into::into).collect(),
		}
	}
}

#[derive(Serialize)]
pub struct TaskReport {
	pub name: Option<String>,
	pub lifecycle: Option<&'static str>,
	pub condition: super::Condition,
	pub provenance: super::Provenance,
	pub record: NativePath,
	pub recorded: Option<SavedTask>,
	pub evidence: Option<SavedTask>,
	pub pending_participant: Option<usize>,
	pub busy: bool,
	pub problems: Vec<ProblemOutcome>,
	pub current: Vec<ParticipantReport>,
}

impl From<TaskSnapshot> for TaskReport {
	fn from(snapshot: TaskSnapshot) -> Self {
		let mut report = Self {
			lifecycle: snapshot
				.task
				.as_ref()
				.or(snapshot.evidence.as_ref())
				.map(|task| task.lifecycle.as_str()),
			condition: super::Condition::Unknown,
			provenance: super::Provenance::Recorded,
			name: snapshot.name,
			record: NativePath::new(snapshot.record).expect("record path"),
			recorded: snapshot.task.map(Into::into),
			evidence: snapshot.evidence.map(Into::into),
			pending_participant: snapshot.pending,
			busy: snapshot.busy,
			problems: snapshot.problems.into_iter().map(Into::into).collect(),
			current: Vec::new(),
		};
		report.classify();
		report
	}
}

impl From<TaskInspection> for TaskReport {
	fn from(inspection: TaskInspection) -> Self {
		let mut report = Self::from(inspection.snapshot);
		report.provenance = super::Provenance::Inspected;
		report.current = inspection
			.participants
			.into_iter()
			.map(Into::into)
			.collect();
		report.classify();
		report
	}
}

#[derive(Serialize, Default)]
pub struct Observation {
	pub state_dir: Option<NativePath>,
	pub tasks_root: Option<NativePath>,
	pub tasks: Vec<TaskReport>,
	pub selected_task: Option<String>,
	pub guidance: Option<String>,
	pub problem: Option<ProblemOutcome>,
}
