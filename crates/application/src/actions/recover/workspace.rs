//! Public outcomes for workspace-wide placement recovery.
use crate::contracts::ProblemOutcome;
use serde::Serialize;
use std::path::Path;
use trine::NativePath;
use workyard_core::{Problem, ProblemCode};

#[derive(Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PlacementState {
	Current,
	RepairNeeded,
	ChoiceRequired,
	Repaired,
	Unresolved,
}

#[derive(Serialize)]
pub struct PlacementReport {
	pub task: Option<String>,
	pub participant: Option<String>,
	pub state: PlacementState,
	pub registered: Option<NativePath>,
	pub destination: Option<NativePath>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub problem: Option<ProblemOutcome>,
}

impl PlacementReport {
	pub fn from_placement(
		task: &str,
		participant: &str,
		placement: &workyard_git::Placement,
	) -> Self {
		let state = if placement.competing {
			PlacementState::ChoiceRequired
		} else if placement.needs_repair {
			PlacementState::RepairNeeded
		} else {
			PlacementState::Current
		};
		Self {
			task: Some(task.to_owned()),
			participant: Some(participant.to_owned()),
			state,
			registered: Some(NativePath::new(&placement.registered).expect("Git path")),
			destination: Some(NativePath::new(&placement.destination).expect("Git path")),
			problem: None,
		}
	}

	pub fn unresolved(task: Option<String>, participant: Option<String>, problem: Problem) -> Self {
		Self {
			task,
			participant,
			state: PlacementState::Unresolved,
			registered: None,
			destination: None,
			problem: Some(problem.into()),
		}
	}
}

#[derive(Default, Serialize)]
pub struct WorkspaceRecovery {
	#[serde(skip)]
	pub(crate) unresolved_locations: bool,
	#[serde(skip)]
	pub(crate) interrupted_removals: Vec<String>,
	#[serde(skip)]
	pub(crate) explicit_workspace: Option<NativePath>,
	pub workspace: Option<NativePath>,
	pub nothing_to_recover: bool,
	pub participants: Vec<PlacementReport>,
	pub problem: Option<Box<ProblemOutcome>>,
}

impl WorkspaceRecovery {
	pub fn at(workspace: &Path) -> Self {
		Self {
			workspace: Some(NativePath::new(workspace).expect("workspace path")),
			..Self::default()
		}
	}

	pub fn failed(mut self, problem: Problem) -> Self {
		self.problem = Some(Box::new(problem.into()));
		self
	}

	pub fn finish(mut self, changed: bool) -> Result<Self, Self> {
		if self.participants.iter().any(|row| row.problem.is_some()) {
			self.unresolved_locations = true;
			return Err(self.failed(Problem::new(
				ProblemCode::RepositoryChanged,
				"Some worktree locations remain unresolved.",
			)));
		}
		self.nothing_to_recover = !changed;
		Ok(self)
	}
}
