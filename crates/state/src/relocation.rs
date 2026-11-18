//! Exclusive workspace movement and paired state authority handoff.
mod admission;
mod content;
mod execution;
mod metadata;
mod observation;
mod record;
mod recovery;
mod transfer;
mod validation;
use crate::{
	Lock,
	documents::{publish, read_versioned},
	records::WorkspaceRecord,
};
pub use observation::{RelocationStatus, relocation_status};
use record::Journal;
use std::path::{Path, PathBuf};
use workyard_core::{Problem, ProblemCode};

pub struct Relocation {
	journal: Journal,
	source_gate: Option<Lock>,
	destination_gate: Option<Lock>,
	started: bool,
}

impl Relocation {
	pub fn old_root(&self) -> Result<PathBuf, Problem> {
		self.journal.old_root.decode()
	}
	pub fn new_root(&self) -> Result<PathBuf, Problem> {
		self.journal.new_root.decode()
	}
	pub fn old_state(&self) -> Result<PathBuf, Problem> {
		self.journal.old_state.decode()
	}
	pub fn new_state(&self) -> Result<PathBuf, Problem> {
		self.journal.new_state.decode()
	}
	pub fn active_root(&self) -> Result<PathBuf, Problem> {
		if self.journal.undone {
			self.old_root()
		} else {
			self.new_root()
		}
	}
	pub fn active_state(&self) -> Result<PathBuf, Problem> {
		if self.journal.undone {
			self.old_state()
		} else {
			self.new_state()
		}
	}
	pub fn placement_complete(&self) -> bool {
		self.journal.activated
	}
	pub fn cleanup_complete(&self) -> bool {
		self.journal.cleanup_complete
	}
	pub fn operation_id(&self) -> &str {
		&self.journal.id
	}

	fn persist(&self) -> Result<(), Problem> {
		let state = if self.journal.activated && !self.journal.undone {
			self.new_state()?
		} else {
			self.old_state()?
		};
		publish(&state.join("relocation.json"), &self.journal)
	}

	fn binding(&self, state: &Path, root: &Path, authority: &str) -> Result<(), Problem> {
		publish(
			&state.join("workspace.json"),
			&WorkspaceRecord {
				version: 2,
				relocation: Some(self.journal.id.clone()),
				authority: Some(authority.into()),
				relocation_source: Some(self.journal.old_state.clone()),
				relocation_destination: Some(self.journal.new_state.clone()),
				tasks_root: root.into(),
				state_dir: state.into(),
			},
		)
	}
}

fn conflict(message: &str) -> Problem {
	Problem::new(ProblemCode::WorkspaceBindingConflict, message).remedy("Inspect both relocation endpoints and preserve existing content. Use recover with the original state selection only for an admitted relocation.")
}

/// A completed marker authorizes normal mutations only at its paired active anchor.
pub(crate) fn validate_mutation_binding(
	state: &Path,
	record: &WorkspaceRecord,
) -> Result<(), Problem> {
	if record
		.authority
		.as_deref()
		.is_some_and(|status| status != "active")
	{
		return Err(conflict(
			"Workspace relocation is unsettled or this state is inactive. Use workspace recover with this state selection.",
		));
	}
	let path = state.join("relocation.json");
	if !path
		.try_exists()
		.map_err(|error| Problem::io("Inspect relocation admission", error))?
	{
		return Ok(());
	}
	let journal: Journal = read_versioned(&path)?;
	journal.validate()?;
	let active = record.relocation.as_deref() == Some(&journal.id)
		&& record.authority.as_deref() == Some("active")
		&& ((state == journal.new_state.decode()?
			&& record.tasks_root.decode()? == journal.new_root.decode()?)
			|| (journal.undone
				&& state == journal.old_state.decode()?
				&& record.tasks_root.decode()? == journal.old_root.decode()?));
	if !active {
		return Err(conflict(
			"Workspace relocation is pending; ordinary mutations are blocked.",
		));
	}
	Ok(())
}
