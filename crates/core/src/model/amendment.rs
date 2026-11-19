use crate::{CreationOptions, Operation, Problem, RepositoryInput, Task};
use std::path::PathBuf;

#[derive(Clone, Debug)]
pub struct AmendRequest {
	pub task: String,
	pub repositories: Vec<RepositoryInput>,
	pub remove: Vec<String>,
	pub removal_options: crate::RemovalOptions,
	pub options: CreationOptions,
	pub remote: Option<String>,
	pub from: Option<String>,
}

/// A selector is bound before any worktree can be renamed or replaced.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RemovalTarget {
	pub name: String,
	pub backing_identity: String,
	pub worktree_identity: String,
}

impl RemovalTarget {
	pub fn matches(&self, participant: &crate::Participant) -> bool {
		self.name == participant.name
			&& self.backing_identity == participant.backing_identity
			&& participant.worktree_identity.as_ref() == Some(&self.worktree_identity)
	}
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
	Removed,
	Renamed,
	Repaired,
	Updated,
	Unchanged,
}

impl AmendmentChangeKind {
	pub fn as_str(self) -> &'static str {
		match self {
			Self::Added => "added",
			Self::Removed => "removed",
			Self::Renamed => "renamed",
			Self::Repaired => "repaired",
			Self::Updated => "updated",
			Self::Unchanged => "unchanged",
		}
	}
}
