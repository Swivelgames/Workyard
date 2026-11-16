use crate::Problem;
use std::path::PathBuf;

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

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Participant {
	pub creation: Option<super::creation::CreationOptions>,
	pub name: String,
	pub source: PathBuf,
	pub common_dir: PathBuf,
	pub backing_identity: String,
	pub worktree_identity: Option<String>,
	pub destination: PathBuf,
	pub remote: Option<String>,
	pub from: Option<String>,
	pub phase: Phase,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TaskLifecycle {
	Active,
	Retired,
}

impl TaskLifecycle {
	pub fn as_str(self) -> &'static str {
		match self {
			Self::Active => "active",
			Self::Retired => "retired",
		}
	}
}

#[derive(Clone, Debug)]
pub struct Task {
	pub generation: u64,
	pub removal: Option<super::removal::Removal>,
	pub name: String,
	pub root: PathBuf,
	pub state_dir: PathBuf,
	pub participants: Vec<Participant>,
	pub checkout: bool,
	pub no_fetch: bool,
	pub pull: bool,
	pub lifecycle: TaskLifecycle,
	pub operation: super::operation::Operation,
	pub problem: Option<Problem>,
}

impl Task {
	pub fn creation_options(&self, index: usize) -> super::creation::CreationOptions {
		self.participants[index]
			.creation
			.unwrap_or(super::creation::CreationOptions {
				checkout: self.checkout,
				no_fetch: self.no_fetch,
				pull: self.pull,
			})
	}
}
