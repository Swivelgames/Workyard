//! Existing-only state reader; observations never enter mutation admission.
mod coordination;
mod history;
mod snapshot;
mod task;
mod validation;
use crate::{Lock, workspace::Workspace};
use std::path::{Path, PathBuf};
use workyard_core::Problem;

/// Shared read access to an existing workspace and its task records.
pub struct Reader {
	pub(crate) root: PathBuf,
	pub(crate) state: PathBuf,
	gate: Lock,
}

impl Reader {
	/// Open the selected workspace and verify its binding.
	pub fn open(state: &Path, requested_root: Option<&Path>) -> Result<Self, Problem> {
		let workspace = Workspace::open(state, requested_root)?;
		Ok(Self {
			root: workspace.root,
			state: workspace.state,
			gate: workspace.gate,
		})
	}

	/// Return the directory containing the task workspaces.
	pub fn root(&self) -> &Path {
		&self.root
	}

	/// Return the workspace state directory.
	pub fn state(&self) -> &Path {
		&self.state
	}

	pub(crate) fn verify(&self) -> Result<(), Problem> {
		Workspace::verify(&self.state, &self.root, &self.gate)
	}
}
