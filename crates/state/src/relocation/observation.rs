//! Read-only relocation inspection never opens mutation admission or redirects it.
use super::{conflict, record::Journal};
use crate::{documents::read_versioned, records::WorkspaceRecord};
use std::path::{Path, PathBuf};
use workyard_core::Problem;

pub struct RelocationStatus {
	pub operation_id: String,
	pub tasks_root: PathBuf,
	pub old_root: PathBuf,
	pub new_root: PathBuf,
	pub old_state: PathBuf,
	pub new_state: PathBuf,
	pub placement_complete: bool,
	pub writer_pending: bool,
	pub authority: String,
}

pub fn relocation_status(state: &Path) -> Result<Option<RelocationStatus>, Problem> {
	let path = state.join("relocation.json");
	let mut gates = Vec::new();
	let (mut journal, inactive): (Journal, bool) = if path
		.try_exists()
		.map_err(|error| Problem::io("Inspect relocation record", error))?
	{
		gates.push(crate::Lock::read_existing(&state.join("workspace.lock"))?);
		(read_versioned(&path)?, false)
	} else {
		let binding = WorkspaceRecord::read(&state.join("workspace.json"))?;
		if binding.authority.as_deref() != Some("inactive") {
			return Ok(None);
		}
		let destination = binding
			.relocation_destination
			.ok_or_else(|| conflict("Inactive source lacks destination routing."))?
			.decode()?;
		gates.push(crate::Lock::read_existing(
			&destination.join("workspace.lock"),
		)?);
		let journal: Journal = read_versioned(&destination.join("relocation.json"))?;
		if binding.relocation.as_deref() != Some(&journal.id) {
			return Err(conflict(
				"Inactive source and destination identities disagree.",
			));
		}
		(journal, true)
	};
	journal.validate()?;
	let old_state = journal.old_state.decode()?;
	let new_state = journal.new_state.decode()?;
	if state != old_state && state != new_state {
		return Err(conflict("Selected state is not a relocation endpoint."));
	}
	if state != old_state
		&& old_state
			.join("relocation.json")
			.try_exists()
			.map_err(|error| Problem::io("Inspect source progress", error))?
	{
		gates.push(crate::Lock::read_existing(
			&old_state.join("workspace.lock"),
		)?);
		let current: Journal = read_versioned(&old_state.join("relocation.json"))?;
		current.validate()?;
		pair(&journal, &current)?;
		journal = current;
	}
	let old_root = journal.old_root.decode()?;
	let new_root = journal.new_root.decode()?;
	let (activation_state, activation_root) = if journal.undone {
		(&old_state, &old_root)
	} else {
		(&new_state, &new_root)
	};

	let selected = binding_at(state)?;
	let authority = selected
		.as_ref()
		.map(|binding| binding.authority.as_deref().unwrap_or("relocating"))
		.unwrap_or(if inactive { "inactive" } else { "pending" })
		.to_owned();

	let activation = if state == activation_state {
		selected
	} else if crate::exists(&activation_state.join("workspace.json"))? {
		gates.push(crate::Lock::read_existing(
			&activation_state.join("workspace.lock"),
		)?);
		let paired: Journal = read_versioned(&activation_state.join("relocation.json"))?;
		pair(&journal, &paired)?;
		binding_at(activation_state)?
	} else {
		None
	};
	let placement_complete = match activation {
		Some(binding)
			if binding.authority.as_deref() == Some("active")
				&& binding.relocation.as_deref() == Some(&journal.id) =>
		{
			if binding.tasks_root.decode()? != *activation_root
				|| binding.state_dir.decode()? != *activation_state
			{
				return Err(conflict(
					"Active workspace binding disagrees with relocation.",
				));
			}
			true
		}
		_ if journal.activated => {
			return Err(conflict(
				"Active workspace binding is missing or has changed.",
			));
		}
		_ => false,
	};
	let tasks_root = if placement_complete {
		activation_root.clone()
	} else {
		old_root.clone()
	};

	for gate in &gates {
		gate.verify()?;
	}
	Ok(Some(RelocationStatus {
		operation_id: journal.id,
		tasks_root,
		old_root,
		new_root,
		old_state,
		new_state,
		placement_complete,
		writer_pending: journal.writer_pending,
		authority,
	}))
}

fn binding_at(state: &Path) -> Result<Option<WorkspaceRecord>, Problem> {
	let path = state.join("workspace.json");
	if crate::exists(&path)? {
		return WorkspaceRecord::read(&path).map(Some);
	}
	Ok(None)
}

fn pair(source: &Journal, destination: &Journal) -> Result<(), Problem> {
	if source.id != destination.id
		|| source.old_root != destination.old_root
		|| source.new_root != destination.new_root
		|| source.old_state != destination.old_state
		|| source.new_state != destination.new_state
	{
		return Err(conflict("Relocation observation endpoints disagree."));
	}
	Ok(())
}
