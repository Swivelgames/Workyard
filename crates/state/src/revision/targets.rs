//! A confirmed unlink authorizes exactly one membership omission.
use super::Revision;
use crate::{damage::damaged, records::task::ParticipantRecord};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use workyard_core::{Problem, RemovalTarget, Task};

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct TargetRecord {
	pub name: String,
	pub backing_identity: String,
	pub worktree_identity: String,
}

impl From<&RemovalTarget> for TargetRecord {
	fn from(target: &RemovalTarget) -> Self {
		Self {
			name: target.name.clone(),
			backing_identity: target.backing_identity.clone(),
			worktree_identity: target.worktree_identity.clone(),
		}
	}
}

impl From<TargetRecord> for RemovalTarget {
	fn from(record: TargetRecord) -> Self {
		Self {
			name: record.name,
			backing_identity: record.backing_identity,
			worktree_identity: record.worktree_identity,
		}
	}
}

impl TargetRecord {
	fn is_valid(&self) -> bool {
		workyard_core::safe_component(&self.name)
			&& !self.worktree_identity.is_empty()
			&& !self.backing_identity.is_empty()
	}

	fn matches(&self, participant: &ParticipantRecord) -> bool {
		self.name == participant.name
			&& self.backing_identity == participant.backing_identity
			&& participant.worktree_identity.as_ref() == Some(&self.worktree_identity)
	}
}

pub(super) fn validate(revision: &Revision) -> Result<(), Problem> {
	let membership = &revision.current.task.participants;
	let mut selected = HashSet::new();
	let mut names = HashSet::new();
	for target in revision.request.iter().flat_map(|request| &request.remove) {
		let known =
			membership.iter().any(|p| target.matches(p)) || revision.removed.contains(target);
		if !target.is_valid()
			|| !known || !selected.insert(&target.backing_identity)
			|| !names.insert(&target.name)
		{
			return Err(damaged("Invalid removal target in amendment evidence."));
		}
	}

	let mut removed = HashSet::new();
	for receipt in &revision.removed {
		// An incoming addition can still fail before its worktree exists. Keep the
		// previous receipt until the same backing has an established membership again.
		let reestablished = membership.iter().any(|p| {
			p.worktree_identity.is_some() && p.backing_identity == receipt.backing_identity
		});
		if !receipt.is_valid() || reestablished || !removed.insert(&receipt.backing_identity) {
			return Err(damaged("Invalid confirmed participant removal."));
		}
	}

	for original in &revision.anchor.participants {
		let retained = membership
			.iter()
			.any(|p| p.backing_identity == original.backing_identity);
		if !retained && !removed.contains(&original.backing_identity) {
			return Err(damaged("Membership revision lost an admitted repository."));
		}
	}
	Ok(())
}

impl Revision {
	pub(crate) fn validate_targets(&self) -> Result<(), Problem> {
		validate(self)
	}

	pub(crate) fn settle_unlink(
		&mut self,
		task: &Task,
		target: &RemovalTarget,
	) -> Result<(), Problem> {
		let bound = TargetRecord::from(target);
		let pending = self
			.current
			.pending
			.and_then(|index| self.current.task.participants.get(index));
		if !self
			.request
			.as_ref()
			.is_some_and(|request| request.remove.contains(&bound))
			|| !pending.is_some_and(|p| bound.matches(p))
			|| task.participants.is_empty()
			|| task.participants.iter().any(|p| target.matches(p))
		{
			return Err(damaged(
				"Confirmed unlink disagrees with its pending participant.",
			));
		}
		let survivors = self
			.current
			.task
			.participants
			.iter()
			.filter(|p| !bound.matches(p));
		if !survivors
			.map(|p| &p.backing_identity)
			.eq(task.participants.iter().map(|p| &p.backing_identity))
		{
			return Err(damaged("Confirmed unlink changed unrelated memberships."));
		}
		self.removed.push(bound);
		self.current.pending = None;
		self.current.task = task.into();
		validate(self)
	}
}
