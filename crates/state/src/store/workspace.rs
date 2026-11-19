use super::Store;
use crate::workspace::Workspace;
use std::path::Path;
use workyard_core::Problem;

impl Store {
	/// Open existing workspace state without creating or repairing any files.
	pub fn open_existing(state: &Path, requested_root: Option<&Path>) -> Result<Self, Problem> {
		let workspace = Workspace::open(state, requested_root)?;
		Ok(Self {
			root: workspace.root,
			state: workspace.state,
			workspace: workspace.gate,
		})
	}

	pub fn verify(&self) -> Result<(), Problem> {
		Workspace::verify(&self.state, &self.root, &self.workspace)
	}
}
