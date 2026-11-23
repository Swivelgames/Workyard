mod participant;
pub use participant::{ParticipantReport, ParticipantState};

use crate::contracts::ProblemOutcome;
use serde::Serialize;
use trine::NativePath;
use workyard_core::{Task, TaskInspection, TaskSnapshot};

/// Whether this report uses recorded facts or a live inspection.
#[derive(Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Provenance {
	/// Derived from saved workspace facts.
	Recorded,
	/// Derived from current inspection.
	Inspected,
}

/// Last recorded lifecycle and participant facts.
#[derive(Serialize)]
pub struct SavedTask {
	/// Recorded locations of the backing repositories.
	pub backing_repositories: std::collections::BTreeMap<String, NativePath>,
	#[serde(skip)]
	pub(crate) repository_sources: std::collections::BTreeMap<String, NativePath>,
	#[serde(skip)]
	pub(crate) removed: std::collections::BTreeSet<String>,
	/// Current membership lifecycle of the task.
	pub lifecycle: &'static str,
	/// Invocation history tracked separately from task lifecycle.
	pub operation: crate::contracts::OperationReport,
	/// Diagnostic describing a failure or an unresolved condition.
	pub problem: Option<ProblemOutcome>,
	/// Members associated with this task or operation.
	pub participants: Vec<crate::contracts::ParticipantOutcome>,
}

impl From<Task> for SavedTask {
	fn from(task: Task) -> Self {
		let removed = task
			.removal
			.as_ref()
			.into_iter()
			.flat_map(|removal| task.participants.iter().zip(&removal.participants))
			.filter(|(_, progress)| progress.phase == workyard_core::RemovalPhase::Removed)
			.map(|(participant, _)| participant.name.clone())
			.collect();
		Self {
			removed,
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
			repository_sources: task
				.participants
				.iter()
				.map(|participant| {
					(
						participant.name.clone(),
						NativePath::new(&participant.source).expect("recorded repository source"),
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

/// Recorded state, current observations and diagnostics for one task.
#[derive(Serialize)]
pub struct TaskReport {
	/// Name used to identify this entry.
	pub name: Option<String>,
	/// Current membership lifecycle of the task.
	pub lifecycle: Option<&'static str>,
	/// User-facing health classification for the observed task.
	pub condition: super::Condition,
	/// Origin of the facts in this report.
	pub provenance: super::Provenance,
	/// Location of the task record when available.
	pub record: NativePath,
	/// Last published task information.
	pub recorded: Option<SavedTask>,
	/// Independent evidence available for checking the task record.
	pub evidence: Option<SavedTask>,
	/// Index of the participant associated with outstanding effect evidence.
	pub pending_participant: Option<usize>,
	/// Whether another operation currently holds admission.
	pub busy: bool,
	/// Diagnostics collected while reading or inspecting the task.
	pub problems: Vec<ProblemOutcome>,
	/// Facts collected from the current filesystem or Git inspection.
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

/// Workspace-scoped result of a task query.
#[derive(Serialize, Default)]
pub struct Observation {
	#[serde(skip)]
	pub(crate) explicit_workspace: Option<NativePath>,
	/// Directory holding authoritative Workyard state.
	pub state_dir: Option<NativePath>,
	/// Directory containing the task workspaces.
	pub tasks_root: Option<NativePath>,
	/// Task reports matching this query.
	pub tasks: Vec<TaskReport>,
	/// Task selector supplied by the caller, if any.
	pub selected_task: Option<String>,
	/// Actionable guidance associated with this result.
	pub guidance: Option<String>,
	/// Diagnostic describing a failure or an unresolved condition.
	pub problem: Option<ProblemOutcome>,
}
