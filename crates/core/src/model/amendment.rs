use crate::{CreationOptions, Operation, Problem, RepositoryInput, Task};
use std::path::PathBuf;

/// Requested changes to an existing task's membership.
#[derive(Clone, Debug)]
pub struct AmendRequest {
	/// Task selected for this operation.
	pub task: String,
	/// Backing repositories selected for membership.
	pub repositories: Vec<RepositoryInput>,
	/// Participant names selected for unlinking.
	pub remove: Vec<String>,
	/// Consent and direction supplied for unlinking worktrees.
	pub removal_options: crate::RemovalOptions,
	/// Branch reuse and network policy for new participants.
	pub options: CreationOptions,
	/// Remote selected for branch resolution or fetch.
	pub remote: Option<String>,
	/// Starting revision for a newly created branch.
	pub from: Option<String>,
}

/// A selector is bound before any worktree can be renamed or replaced.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RemovalTarget {
	/// Name used to identify this entry.
	pub name: String,
	/// Identity used to distinguish backing repositories across path changes.
	pub backing_identity: String,
	/// Identity used to verify the linked worktree before applying changes.
	pub worktree_identity: String,
}

impl RemovalTarget {
	/// Verify the bound name and both identities against the current participant.
	pub fn matches(&self, participant: &crate::Participant) -> bool {
		self.name == participant.name
			&& self.backing_identity == participant.backing_identity
			&& participant.worktree_identity.as_ref() == Some(&self.worktree_identity)
	}
}

/// A participant and the effect that amendment had on it.
#[derive(Clone, Debug)]
pub struct AmendmentChange {
	/// Participant to which this observation or change applies.
	pub participant: String,
	/// Classification used to interpret this record.
	pub kind: AmendmentChangeKind,
}

/// Membership and publication facts available after amendment.
pub struct AmendResult {
	/// Newly established project roots eligible for catalog registration.
	pub admitted_roots: Vec<PathBuf>,
	/// Whether the invocation entered the mutation phase.
	pub mutation_started: bool,
	/// Task selected for this operation.
	pub task: Task,
	/// Location of the task record when available.
	pub record: PathBuf,
	/// Invocation history tracked separately from task lifecycle.
	pub operation: Operation,
	/// Changes reported for individual participants.
	pub changes: Vec<AmendmentChange>,
	/// Diagnostic describing a failure or an unresolved condition.
	pub problem: Option<Problem>,
}

/// How an amendment affected a participant.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AmendmentChangeKind {
	/// The participant joined the task.
	Added,
	/// The worktree was unlinked and membership removed.
	Removed,
	/// The participant received a new task-local name.
	Renamed,
	/// Git registration was reconciled with the worktree.
	Repaired,
	/// Recorded participant details changed.
	Updated,
	/// Membership and placement required no update.
	Unchanged,
}

impl AmendmentChangeKind {
	/// Return the stable spelling used in records or output.
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
