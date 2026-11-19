//! Resolve requested identities before validating the final participant layout.
mod layout;
mod removals;
mod selection;
use layout::{order_moves, validate_layout};

use crate::{
	AmendGit, AmendRequest, AmendState, Control, Participant, Problem, ProblemCode, Repository,
	Task,
};
use std::collections::HashMap;

#[derive(Clone)]
pub(super) struct Change {
	pub repository: Repository,
	pub participant: Participant,
	pub existing: Option<usize>,
	pub name: String,
	pub explicit_name: bool,
	pub name_source: std::path::PathBuf,
	pub repair: bool,
	pub addition: bool,
}

pub(super) struct Plan {
	pub changes: Vec<Change>,
	pub targets: Vec<crate::RemovalTarget>,
}

pub(super) async fn plan<S: AmendState>(
	request: &AmendRequest,
	task: &Task,
	recovering: bool,
	git: &impl AmendGit,
	state: &S,
	guard: &S::Admission,
	control: &Control,
) -> Result<Plan, Problem> {
	let mut changes: Vec<Change> = Vec::new();
	let mut identities = HashMap::new();
	for input in &request.repositories {
		control.check()?;
		let change = Change::prepare(request, task, input, recovering, git, control).await?;
		if let Some(index) = identities.get(&change.repository.identity) {
			let previous: &mut Change = &mut changes[*index];
			validate_alias(previous, &change)?;
			previous.explicit_name |= change.explicit_name;
			previous
				.participant
				.registration_roots
				.extend(input.registration_roots.iter().cloned());
			continue;
		}

		identities.insert(change.repository.identity.clone(), changes.len());
		changes.push(change);
	}

	let targets = removals::bind(request, task, &mut changes, recovering, state, guard)?;
	if changes.is_empty()
		&& task
			.participants
			.iter()
			.all(|p| targets.iter().any(|t| t.matches(p)))
	{
		return Err(Problem::new(
			ProblemCode::EmptyTask,
			"An active task must retain at least one participant.",
		)
		.remedy("Use workyard remove to retire the entire task."));
	}

	if recovering {
		// The request retains unfinished intentions; authoritative membership supplies
		// recovery coverage for participants omitted by a later amendment.
		for participant in &task.participants {
			if identities.contains_key(&participant.backing_identity)
				|| targets.iter().any(|t| t.matches(participant))
			{
				continue;
			}
			let input = crate::RepositoryInput {
				registration_roots: Vec::new(),
				expected_identity: None,
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

	layout::allocate_names(task, &mut changes, &targets)?;
	validate_layout(task, &changes, &targets, state)?;
	Ok(Plan {
		changes: order_moves(task, changes)?,
		targets,
	})
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
