use crate::contracts::ProblemOutcome;
use serde::Serialize;
use trine::NativePath;
use workyard_core::ParticipantInspection;

/// User-facing assessment of one participant worktree.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ParticipantState {
	/// Git reports a clean worktree.
	Clean,
	/// The worktree contains local changes.
	Modified,
	/// Git reports unresolved conflicts.
	Conflicted,
	/// The participant worktree could not be located.
	Missing,
	/// Inspection could not establish current Git state.
	Unavailable,
}

impl ParticipantState {
	/// Return the human-readable status label.
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

/// Current Git facts alongside the participant's recorded placement.
#[derive(Serialize)]
pub struct ParticipantReport {
	/// Name used to identify this entry.
	pub name: String,
	/// Location of the backing Git repository.
	pub backing_repo: Option<NativePath>,
	#[serde(skip)]
	pub(crate) repository_source: Option<NativePath>,
	/// Assessment of the current worktree from inspection.
	pub state: ParticipantState,
	/// Worktree location stored in task state.
	pub recorded_path: NativePath,
	/// Worktree location discovered during inspection.
	pub current_path: Option<NativePath>,
	/// Branch associated with the worktree, when known.
	pub branch: Option<String>,
	/// Whether the worktree has local changes.
	pub dirty: Option<bool>,
	/// Whether Git reports unresolved conflicts.
	pub conflicted: Option<bool>,
	/// Whether an unfinished repository operation was observed.
	pub operation_in_progress: Option<bool>,
	/// Diagnostic describing a failure or an unresolved condition.
	pub problem: Option<ProblemOutcome>,
}

impl From<ParticipantInspection> for ParticipantReport {
	fn from(inspection: ParticipantInspection) -> Self {
		let mut participant = Self {
			name: inspection.participant.name,
			backing_repo: None,
			repository_source: None,
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
				participant.backing_repo = facts
					.common_dir
					.map(|path| NativePath::new(path).expect("Git common path"));
				participant.repository_source = facts
					.repository_source
					.map(|path| NativePath::new(path).expect("Git repository source"));
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
