use crate::{CreationOptions, Operation, Problem, RepositoryInput, Task};
use std::path::PathBuf;

#[derive(Clone, Debug)]
pub struct AmendRequest {
	pub task: String,
	pub repositories: Vec<RepositoryInput>,
	pub options: CreationOptions,
	pub remote: Option<String>,
	pub from: Option<String>,
}

#[derive(Clone, Debug)]
pub struct AmendmentChange {
	pub participant: String,
	pub kind: AmendmentChangeKind,
}

pub struct AmendResult {
	pub admitted_roots: Vec<PathBuf>,
	pub pull_failures: Vec<crate::PullFailure>,
	pub mutation_started: bool,
	pub task: Task,
	pub record: PathBuf,
	pub operation: Operation,
	pub changes: Vec<AmendmentChange>,
	pub problem: Option<Problem>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AmendmentChangeKind {
	Added,
	Renamed,
	Repaired,
	Updated,
	Unchanged,
}

impl AmendmentChangeKind {
	pub fn as_str(self) -> &'static str {
		match self {
			Self::Added => "added",
			Self::Renamed => "renamed",
			Self::Repaired => "repaired",
			Self::Updated => "updated",
			Self::Unchanged => "unchanged",
		}
	}
}
