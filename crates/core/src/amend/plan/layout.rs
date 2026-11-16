use super::Change;
use crate::{AmendState, Problem, ProblemCode, Task};
use std::collections::HashSet;

pub(super) fn validate_layout(
	task: &Task,
	changes: &[Change],
	state: &impl AmendState,
) -> Result<(), Problem> {
	let mut names = HashSet::new();
	for (index, participant) in task.participants.iter().enumerate() {
		let name = changes
			.iter()
			.find(|change| change.existing == Some(index))
			.map_or(participant.name.as_str(), |change| &change.name);
		insert_name(&mut names, name)?;
	}
	for change in changes {
		if change.existing.is_none() {
			insert_name(&mut names, &change.name)?;
		}
		let destination = task.root.join(&task.name).join(&change.name);
		if !change.addition && change.name == change.participant.name {
			continue;
		}
		if !change.addition && destination == change.participant.destination {
			continue;
		}
		let vacated = changes.iter().any(|other| {
			!other.addition
				&& other.name != other.participant.name
				&& other.participant.destination == destination
		});
		if !vacated {
			state.validate_destination(&destination)?;
		}
	}
	Ok(())
}

fn insert_name(names: &mut HashSet<String>, name: &str) -> Result<(), Problem> {
	#[cfg(any(windows, target_os = "macos"))]
	let key = name.to_uppercase().to_lowercase();
	#[cfg(not(any(windows, target_os = "macos")))]
	let key = name.to_owned();
	if !names.insert(key) {
		return Err(Problem::new(
			ProblemCode::ParticipantNameConflict,
			format!("Participant name {name:?} is not unique in the amended task."),
		));
	}
	Ok(())
}

pub(super) fn order_moves(task: &Task, mut changes: Vec<Change>) -> Result<Vec<Change>, Problem> {
	let mut ordered = Vec::new();
	while !changes.is_empty() {
		let next = changes.iter().position(|candidate| {
			let destination = task.root.join(&task.name).join(&candidate.name);
			!changes.iter().any(|other| {
				other.repository.identity != candidate.repository.identity
					&& !other.addition
					&& other.participant.destination == destination
					&& other.name != other.participant.name
			})
		});
		let Some(index) = next else {
			return Err(Problem::new(
				ProblemCode::PathOverlap,
				"Participant renames form a cycle. Rename one participant to an unused name first.",
			));
		};
		ordered.push(changes.remove(index));
	}
	ordered.sort_by_key(|change| change.addition);
	Ok(ordered)
}
