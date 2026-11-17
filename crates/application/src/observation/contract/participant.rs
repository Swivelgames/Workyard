use crate::contracts::ProblemOutcome;
use serde::Serialize;
use trine::NativePath;
use workyard_core::ParticipantInspection;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ParticipantState {
	Clean,
	Modified,
	Conflicted,
	Missing,
	Unavailable,
}

impl ParticipantState {
	pub fn label(self) -> &'static str {
		match self {
			Self::Clean => "clean",
			Self::Modified => "modified",
			Self::Conflicted => "conflicted",
			Self::Missing => "missing",
			Self::Unavailable => "unavailable",
		}
	}
}

#[derive(Serialize)]
pub struct ParticipantReport {
	pub name: String,
	pub state: ParticipantState,
	pub recorded_path: NativePath,
	pub current_path: Option<NativePath>,
	pub branch: Option<String>,
	pub dirty: Option<bool>,
	pub conflicted: Option<bool>,
	pub operation_in_progress: Option<bool>,
	pub problem: Option<ProblemOutcome>,
}

impl From<ParticipantInspection> for ParticipantReport {
	fn from(inspection: ParticipantInspection) -> Self {
		let mut participant = Self {
			name: inspection.participant.name,
			state: ParticipantState::Unavailable,
			recorded_path: NativePath::new(inspection.participant.destination)
				.expect("participant path"),
			current_path: None,
			branch: None,
			dirty: None,
			conflicted: None,
			operation_in_progress: None,
			problem: None,
		};
		match inspection.current {
			Ok(facts) => {
				participant.state = if facts.path.is_none() {
					ParticipantState::Missing
				} else if facts.conflicted {
					ParticipantState::Conflicted
				} else if facts.dirty {
					ParticipantState::Modified
				} else {
					ParticipantState::Clean
				};
				participant.problem = facts.problem.map(Into::into);
				participant.current_path = facts
					.path
					.map(|path| NativePath::new(path).expect("Git path"));
				participant.branch = facts.branch;
				participant.dirty = participant.current_path.as_ref().map(|_| facts.dirty);
				participant.conflicted =
					participant.current_path.as_ref().map(|_| facts.conflicted);
				participant.operation_in_progress = participant
					.current_path
					.as_ref()
					.map(|_| facts.operation_in_progress);
			}
			Err(problem) => participant.problem = Some(problem.into()),
		}
		participant
	}
}
