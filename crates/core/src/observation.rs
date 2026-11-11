//! Read-only task observations keep saved progress distinct from current Git facts.
use crate::{Control, Participant, Problem, Task};
use std::{future::Future, path::PathBuf};

pub struct TaskSnapshot {
	pub name: Option<String>,
	pub record: PathBuf,
	pub task: Option<Task>,
	pub evidence: Option<Task>,
	pub pending: Option<usize>,
	pub busy: bool,
	pub problems: Vec<Problem>,
}

pub struct GitFacts {
	/// An observed condition can be known (such as absence) and still need a remedy.
	pub problem: Option<Problem>,
	pub path: Option<PathBuf>,
	pub branch: Option<String>,
	pub dirty: bool,
	pub conflicted: bool,
	pub operation_in_progress: bool,
}

pub struct ParticipantInspection {
	pub participant: Participant,
	pub current: Result<GitFacts, Problem>,
}

pub struct TaskInspection {
	pub snapshot: TaskSnapshot,
	pub participants: Vec<ParticipantInspection>,
}

pub trait ObservationGit: Sync {
	fn observe(
		&self,
		participant: &Participant,
		control: &Control,
	) -> impl Future<Output = Result<GitFacts, Problem>> + Send;
}

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
