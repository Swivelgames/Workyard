use crate::path::StoredPath;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use workyard_core::{Problem, Task};

pub fn task_filename(name: &str) -> String {
	let mut slug = String::new();
	for character in name.chars() {
		if character.is_ascii_alphanumeric() {
			slug.push(character.to_ascii_lowercase());
		} else if !slug.is_empty() && !slug.ends_with('-') {
			slug.push('-');
		}
	}
	slug.truncate(slug.len().min(48));
	let slug = slug.trim_end_matches('-');
	let slug = if slug.is_empty() { "task" } else { slug };
	let hash: String = Sha256::digest(name.as_bytes())
		.iter()
		.map(|byte| format!("{byte:02x}"))
		.collect();
	format!("{slug}-{hash}.json")
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorkspaceRecord {
	pub version: u32,
	pub tasks_root: StoredPath,
	pub state_dir: StoredPath,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ParticipantRecord {
	pub name: String,
	pub source: StoredPath,
	pub common_dir: StoredPath,
	pub backing_identity: String,
	#[serde(default)]
	pub worktree_identity: Option<String>,
	pub destination: StoredPath,
	pub remote: Option<String>,
	pub from: Option<String>,
	pub phase: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ProblemRecord {
	pub code: String,
	pub message: String,
	pub remedy: String,
}

impl From<&Problem> for ProblemRecord {
	fn from(problem: &Problem) -> Self {
		Self {
			code: problem.code.as_str().into(),
			message: problem.message.clone(),
			remedy: problem.remedy.clone(),
		}
	}
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TaskRecord {
	pub version: u32,
	pub task: String,
	pub tasks_root: StoredPath,
	pub state_dir: StoredPath,
	pub checkout: bool,
	pub no_fetch: bool,
	pub pull: bool,
	pub status: String,
	pub participants: Vec<ParticipantRecord>,
	pub problem: Option<ProblemRecord>,
}

impl From<&Task> for TaskRecord {
	fn from(task: &Task) -> Self {
		Self {
			version: 1,
			task: task.name.clone(),
			tasks_root: task.root.as_path().into(),
			state_dir: task.state_dir.as_path().into(),
			checkout: task.checkout,
			no_fetch: task.no_fetch,
			pull: task.pull,
			status: task.status.as_str().into(),
			problem: task.problem.as_ref().map(Into::into),
			participants: task
				.participants
				.iter()
				.map(|participant| ParticipantRecord {
					name: participant.name.clone(),
					source: participant.source.as_path().into(),
					common_dir: participant.common_dir.as_path().into(),
					backing_identity: participant.backing_identity.clone(),
					worktree_identity: participant.worktree_identity.clone(),
					destination: participant.destination.as_path().into(),
					remote: participant.remote.clone(),
					from: participant.from.clone(),
					phase: participant.phase.as_str().into(),
				})
				.collect(),
		}
	}
}
