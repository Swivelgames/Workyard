//! Read-only task observations keep saved progress distinct from current Git facts.
use crate::{Control, Participant, Problem, Task};
use std::{future::Future, path::PathBuf};

/// Recorded task and evidence available during read-only inspection.
pub struct TaskSnapshot {
	/// Name used to identify this entry.
	pub name: Option<String>,
	/// Location of the task record when available.
	pub record: PathBuf,
	/// Decoded task when the published record is readable.
	pub task: Option<Task>,
	/// Independent evidence available for checking the task record.
	pub evidence: Option<Task>,
	/// Participant index associated with unresolved effect evidence.
	pub pending: Option<usize>,
	/// Whether another operation currently holds admission.
	pub busy: bool,
	/// Diagnostics collected while reading or inspecting the task.
	pub problems: Vec<Problem>,
}

/// Current branch, worktree location and working-copy observations from Git.
pub struct GitFacts {
	/// Verified common Git storage, not the linked worktree administration.
	pub common_dir: Option<PathBuf>,
	/// An observed condition can be known (such as absence) and still need a remedy.
	pub problem: Option<Problem>,
	/// Worktree location reported by Git, when available.
	pub path: Option<PathBuf>,
	/// Branch associated with the worktree, when known.
	pub branch: Option<String>,
	/// Whether the worktree has local changes.
	pub dirty: bool,
	/// Whether Git reports unresolved conflicts.
	pub conflicted: bool,
	/// Whether an unfinished repository operation was observed.
	pub operation_in_progress: bool,
}

/// Recorded participant paired with its current observation.
pub struct ParticipantInspection {
	/// Participant to which this observation or change applies.
	pub participant: Participant,
	/// Facts collected from the current filesystem or Git inspection.
	pub current: Result<GitFacts, Problem>,
}

/// Snapshot and live participant observations for a task.
pub struct TaskInspection {
	/// Task and evidence as read before inspecting participants.
	pub snapshot: TaskSnapshot,
	/// Members associated with this task or operation.
	pub participants: Vec<ParticipantInspection>,
}

/// Read-only Git inspection used by status and recovery decisions.
pub trait ObservationGit: Sync {
	/// Collect live repository facts for a recorded participant.
	fn observe(
		&self,
		participant: &Participant,
		control: &Control,
	) -> impl Future<Output = Result<GitFacts, Problem>> + Send;
}

/// Inspect the selected repository or task without applying changes.
pub async fn inspect(
	snapshot: TaskSnapshot,
	git: &impl ObservationGit,
	control: &Control,
) -> Result<TaskInspection, Problem> {
	let mut participants = Vec::new();
	// Independent evidence can describe intended membership when publication failed.
	// It remains separately identified in the result, never reconstructed as a record.
	if let Some(task) = snapshot.task.as_ref().or(snapshot.evidence.as_ref()) {
		for participant in &task.participants {
			control.check()?;
			let current = git.observe(participant, control).await;
			control.check()?;
			participants.push(ParticipantInspection {
				participant: participant.clone(),
				current,
			});
		}
	}
	Ok(TaskInspection {
		snapshot,
		participants,
	})
}
