use super::Change;
use crate::{AmendState, Problem, ProblemCode, Task};
use std::collections::HashSet;

pub(super) fn allocate_names(task: &Task, changes: &mut [Change]) -> Result<(), Problem> {
	use crate::projects::names::{self, NameRequest};
	let selected: HashSet<_> = changes
		.iter()
		.filter_map(|change| change.existing)
		.collect();
	let reserved = task
		.participants
		.iter()
		.enumerate()
		.filter(|(index, _)| !selected.contains(index))
		.map(|(_, p)| (p.name.as_str(), p.source.as_path()));
	let requested: Vec<_> = changes
		.iter()
		.map(|change| NameRequest {
			path: &change.name_source,
			name: &change.name,
			fixed: change.fixed_name,
		})
		.collect();
	let names = names::allocate(&requested, reserved)?;
	for (change, name) in changes.iter_mut().zip(names) {
		if change.addition {
			change.participant.name = name.clone();
			change.participant.destination = task.root.join(&task.name).join(&name);
		}
		change.name = name;
	}
	Ok(())
}

pub(super) fn validate_layout(
	task: &Task,
	changes: &[Change],
	state: &impl AmendState,
) -> Result<(), Problem> {
	for change in changes {
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
