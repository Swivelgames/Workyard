use serde::Serialize;
use trine::{Cancellation, NativePath};

/// Workspace and tasks directory established by initialization.
#[derive(Serialize)]
pub struct InitOutcome {
	#[serde(skip)]
	pub(crate) cancellation: Cancellation,
	/// Workspace root containing the Workyard marker.
	pub workspace: NativePath,
	/// Directory containing the task workspaces.
	pub tasks_root: NativePath,
}
