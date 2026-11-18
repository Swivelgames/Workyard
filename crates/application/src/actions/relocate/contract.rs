use crate::contracts::{OperationReport, ProblemOutcome};
use serde::Serialize;
use trine::NativePath;
use workyard_core::{OperationOutcome, Problem};

#[derive(Serialize)]
pub struct ExternalParticipant {
	pub task: String,
	pub participant: String,
	pub location: NativePath,
}

#[derive(Serialize)]
pub struct RelocateOutcome {
	pub operation: OperationReport,
	pub scope: &'static str,
	pub operation_id: Option<String>,
	pub old_tasks_root: Option<NativePath>,
	pub new_tasks_root: Option<NativePath>,
	pub old_state_dir: Option<NativePath>,
	pub new_state_dir: Option<NativePath>,
	pub active_tasks_root: Option<NativePath>,
	pub active_state_dir: Option<NativePath>,
	pub placement_complete: bool,
	pub cleanup_complete: bool,
	pub nothing_to_recover: bool,
	pub warnings: Vec<ExternalParticipant>,
	pub mutation_started: bool,
	pub problem: Option<ProblemOutcome>,
}

impl RelocateOutcome {
	pub(super) fn empty(kind: &'static str) -> Self {
		Self {
			operation: OperationReport {
				kind: Some(kind),
				continues: (kind == "recover").then_some("relocate"),
				outcome: OperationOutcome::Succeeded,
			},
			scope: "workspace",
			operation_id: None,
			old_tasks_root: None,
			new_tasks_root: None,
			old_state_dir: None,
			new_state_dir: None,
			active_tasks_root: None,
			active_state_dir: None,
			placement_complete: false,
			cleanup_complete: false,
			nothing_to_recover: false,
			warnings: Vec::new(),
			mutation_started: false,
			problem: None,
		}
	}
	pub(crate) fn rejected(kind: &'static str, problem: Problem) -> Self {
		let mut outcome = Self::empty(kind);
		outcome.fail(problem);
		outcome
	}
	pub(super) fn fail(&mut self, problem: Problem) {
		self.operation.outcome = if problem.code == workyard_core::ProblemCode::Cancelled {
			OperationOutcome::Cancelled
		} else {
			OperationOutcome::Failed
		};
		self.problem = Some(problem.into());
	}
}
