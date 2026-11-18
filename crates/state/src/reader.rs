//! Existing-only state reader; observations never enter mutation admission.
mod coordination;
mod history;
mod snapshot;
mod task;
mod validation;
use crate::{Lock, canonical_future, records::WorkspaceRecord};
use std::path::{Path, PathBuf};
use workyard_core::{Problem, ProblemCode};

pub struct Reader {
	pub(crate) root: PathBuf,
	pub(crate) state: PathBuf,
	gate: Lock,
}

impl Reader {
	pub fn open(state: &Path, requested_root: Option<&Path>) -> Result<Self, Problem> {
		let state = canonical_future(state)?;
		let gate = Lock::read_existing(&state.join("workspace.lock"))?;
		let record: WorkspaceRecord = WorkspaceRecord::read(&state.join("workspace.json"))?;
		let root = record.tasks_root.decode()?;
		if !root.is_absolute()
			|| record.state_dir.decode()? != state
			|| canonical_future(&root)? != root
		{
			return Err(crate::damage::damaged(
				"Workspace binding disagrees with its state location.",
			));
		}
		if let Some(requested) = requested_root
			&& canonical_future(requested)? != root
		{
			return Err(Problem::new(
				ProblemCode::WorkspaceBindingConflict,
				"Requested tasks root differs from selected state.",
			));
		}
		for name in ["tasks", "evidence", "locks"] {
			if !state.join(name).is_dir() {
				return Err(crate::damage::damaged(format!(
					"State directory is missing {name}."
				)));
			}
		}
		let reader = Self { root, state, gate };
		reader.verify()?;
		Ok(reader)
	}

	pub fn root(&self) -> &Path {
		&self.root
	}

	pub fn state(&self) -> &Path {
		&self.state
	}

	pub(crate) fn verify(&self) -> Result<(), Problem> {
		self.gate.verify()?;
		let record: WorkspaceRecord = WorkspaceRecord::read(&self.state.join("workspace.json"))?;
		if record.tasks_root.decode()? != self.root || record.state_dir.decode()? != self.state {
			return Err(crate::damage::damaged(
				"Workspace binding changed during observation.",
			));
		}
		Ok(())
	}
}
