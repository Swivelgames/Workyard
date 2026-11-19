use super::Change;
use crate::{AmendRequest, AmendState, Problem, ProblemCode, RemovalTarget, Task};
use std::collections::HashSet;

pub(super) fn bind<S: AmendState>(
	request: &AmendRequest,
	task: &Task,
	changes: &mut [Change],
	recovering: bool,
	state: &S,
	guard: &S::Admission,
) -> Result<Vec<RemovalTarget>, Problem> {
	if request.remove.is_empty() {
		return Ok(Vec::new());
	}

	// Replaying a replacement must keep referring to the original "api", even if
	// a different repository now occupies that name and directory.
	let previous = state.amend_request(guard)?;
	let previous = previous.filter(|previous| {
		same_names(&request.remove, &previous.remove)
			&& (recovering || same_selection(changes, previous))
	});
	let targets = if let Some(previous) = previous {
		for change in changes.iter_mut() {
			let saved_name = previous
				.repositories
				.iter()
				.find(|repo| repo.expected_identity.as_ref() == Some(&change.repository.identity))
				.and_then(|repo| repo.name.as_ref());
			if let Some(name) = saved_name {
				change.name.clone_from(name);
				change.explicit_name = true;
			}
		}
		state.removal_targets(guard)?
	} else {
		from_membership(task, &request.remove)?
	};
	for target in &targets {
		if changes
			.iter()
			.any(|change| change.repository.identity == target.backing_identity)
		{
			return Err(Problem::new(
				ProblemCode::InvalidInput,
				format!(
					"Participant {:?} cannot be removed and included in the same amendment.",
					target.name
				),
			));
		}
	}
	Ok(targets)
}

fn same_names(left: &[String], right: &[String]) -> bool {
	left.iter().collect::<HashSet<_>>() == right.iter().collect::<HashSet<_>>()
}

fn same_selection(changes: &[Change], previous: &AmendRequest) -> bool {
	changes.len() == previous.repositories.len()
		&& changes.iter().all(|change| {
			previous.repositories.iter().any(|repo| {
				repo.expected_identity.as_ref() == Some(&change.repository.identity)
					&& (!change.explicit_name || repo.name.as_ref() == Some(&change.name))
			})
		})
}

fn from_membership(task: &Task, names: &[String]) -> Result<Vec<RemovalTarget>, Problem> {
	let mut targets = Vec::new();
	let mut selected = HashSet::new();
	for name in names {
		if !selected.insert(name) {
			continue;
		}
		let participant = task
			.participants
			.iter()
			.find(|p| &p.name == name)
			.ok_or_else(|| {
				Problem::new(
					ProblemCode::InvalidInput,
					format!("Task {:?} has no participant {name:?}.", task.name),
				)
			})?;
		let identity = participant.worktree_identity.clone().ok_or_else(|| {
			Problem::new(
				ProblemCode::RepositoryChanged,
				format!("Worktree identity for {name:?} is unconfirmed."),
			)
		})?;
		targets.push(RemovalTarget {
			name: name.clone(),
			backing_identity: participant.backing_identity.clone(),
			worktree_identity: identity,
		});
	}
	Ok(targets)
}
