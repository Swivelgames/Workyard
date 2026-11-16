//! Resolve requested identities before validating the final participant layout.
mod layout;
mod selection;
use layout::{order_moves, validate_layout};

use crate::{
	AmendGit, AmendRequest, AmendState, Control, Participant, Problem, ProblemCode, Repository,
	Task,
};
use std::collections::HashMap;

pub(super) struct Change {
	pub repository: Repository,
	pub participant: Participant,
	pub existing: Option<usize>,
	pub name: String,
	pub repair: bool,
	pub addition: bool,
}

pub(super) async fn plan(
	request: &AmendRequest,
	task: &Task,
	recovering: bool,
	git: &impl AmendGit,
	state: &impl AmendState,
	control: &Control,
) -> Result<Vec<Change>, Problem> {
	let mut changes: Vec<Change> = Vec::new();
	let mut identities = HashMap::new();
	for input in &request.repositories {
		control.check()?;
		let change = Change::prepare(request, task, input, recovering, git, control).await?;
		if let Some(index) = identities.get(&change.repository.identity) {
			validate_alias(&changes[*index], &change)?;
			continue;
		}

		identities.insert(change.repository.identity.clone(), changes.len());
		changes.push(change);
	}

	if recovering {
		// The request retains unfinished intentions; authoritative membership supplies
		// recovery coverage for participants omitted by a later amendment.
		for participant in &task.participants {
			if identities.contains_key(&participant.backing_identity) {
				continue;
			}
			let input = crate::RepositoryInput {
				path: super::admission::locate(participant, git, control).await?,
				name: Some(participant.name.clone()),
				remote: participant.remote.clone(),
				from: participant.from.clone(),
			};
			let participant_request = AmendRequest {
				remote: None,
				from: None,
				..request.clone()
			};
			changes.push(
				Change::prepare(&participant_request, task, &input, true, git, control).await?,
			);
		}
	}

	validate_layout(task, &changes, state)?;
	order_moves(task, changes)
}

fn validate_alias(previous: &Change, current: &Change) -> Result<(), Problem> {
	if previous.name == current.name
		&& previous.repository.remote == current.repository.remote
		&& (!previous.addition || previous.participant.from == current.participant.from)
	{
		return Ok(());
	}

	Err(Problem::new(
		ProblemCode::RepositoryAliasConflict,
		"Repeated repository has conflicting names or options.",
	))
}
