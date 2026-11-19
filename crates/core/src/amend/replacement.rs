//! Establish incoming memberships before unlinking incumbents. When the incoming
//! name is occupied, Git can create the worktree under a temporary task-local name.
use super::plan::Change;
use crate::{AmendState, Problem, Task};
use std::{collections::HashSet, path::Path};

pub(super) struct Schedule {
	pub before: Vec<Change>,
	pub after: Vec<Change>,
}

impl Schedule {
	pub fn prepare(
		task: &Task,
		changes: Vec<Change>,
		removing: bool,
		state: &impl AmendState,
	) -> Result<Self, Problem> {
		if !removing {
			return Ok(Self {
				before: changes,
				after: Vec::new(),
			});
		}
		let mut used: HashSet<_> = task
			.participants
			.iter()
			.map(|p| p.name.to_lowercase())
			.chain(changes.iter().map(|c| c.name.to_lowercase()))
			.collect();
		let mut before = Vec::new();
		let mut after = Vec::new();
		let task_directory = task.root.join(&task.name);
		for change in changes {
			if !change.addition {
				after.push(change);
				continue;
			}

			let destination = task_directory.join(&change.name);
			if !occupied(task, &change, &destination, state)? {
				before.push(change);
				continue;
			}

			let temporary = staging_name(&mut used, &task_directory, state)?;
			let mut incoming = change.clone();
			incoming.participant.name = temporary.clone();
			incoming.participant.destination = task_directory.join(&temporary);
			before.push(incoming);

			let mut rename = change;
			rename.addition = false;
			after.push(rename);
		}
		Ok(Self { before, after })
	}
}

fn occupied(
	task: &Task,
	change: &Change,
	destination: &Path,
	state: &impl AmendState,
) -> Result<bool, Problem> {
	for incumbent in &task.participants {
		if incumbent.backing_identity == change.repository.identity {
			continue;
		}
		if incumbent.name == change.name
			|| state.same_destination(&incumbent.destination, destination)?
		{
			return Ok(true);
		}
	}
	Ok(false)
}

fn staging_name(
	used: &mut HashSet<String>,
	directory: &Path,
	state: &impl AmendState,
) -> Result<String, Problem> {
	for suffix in 1u64.. {
		let name = format!(".workyard-incoming-{suffix}");
		if used.insert(name.clone()) && state.staging_available(&directory.join(&name))? {
			return Ok(name);
		}
	}
	unreachable!("finite workspace directory")
}
