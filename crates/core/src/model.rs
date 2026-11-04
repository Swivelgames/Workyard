use crate::Problem;
use std::path::PathBuf;

#[derive(Clone, Debug)]
pub struct RepositoryInput {
	pub path: PathBuf,
	pub name: Option<String>,
	pub remote: Option<String>,
	pub from: Option<String>,
}

#[derive(Clone, Debug)]
pub struct CreateRequest {
	pub task: String,
	pub repositories: Vec<RepositoryInput>,
	pub tasks_root: PathBuf,
	pub state_dir: PathBuf,
	pub checkout: bool,
	pub no_fetch: bool,
	pub pull: bool,
	pub remote: Option<String>,
	pub from: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Phase {
	NotStarted,
	Fetching,
	Creating,
	WorktreeCreated,
	Pulling,
	Completed,
}

impl Phase {
	pub fn as_str(&self) -> &'static str {
		match self {
			Self::NotStarted => "not_started",
			Self::Fetching => "fetching",
			Self::Creating => "creating",
			Self::WorktreeCreated => "worktree_created",
			Self::Pulling => "pulling",
			Self::Completed => "completed",
		}
	}
}

#[derive(Clone, Debug)]
pub struct Participant {
	pub name: String,
	pub source: PathBuf,
	pub common_dir: PathBuf,
	pub backing_identity: String,
	pub destination: PathBuf,
	pub remote: Option<String>,
	pub from: Option<String>,
	pub phase: Phase,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TaskStatus {
	Pending,
	Completed,
	Cancelled,
	Failed,
}

impl TaskStatus {
	pub fn as_str(self) -> &'static str {
		match self {
			Self::Pending => "pending",
			Self::Completed => "completed",
			Self::Cancelled => "cancelled",
			Self::Failed => "failed",
		}
	}
}

#[derive(Clone, Debug)]
pub struct Task {
	pub name: String,
	pub root: PathBuf,
	pub state_dir: PathBuf,
	pub participants: Vec<Participant>,
	pub checkout: bool,
	pub no_fetch: bool,
	pub pull: bool,
	pub status: TaskStatus,
	pub problem: Option<Problem>,
}
