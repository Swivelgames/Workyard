use crate::contracts::ProblemOutcome;
use serde::Serialize;
use trine::NativePath;

/// How this invocation selected the workspace.
#[derive(Clone, Copy, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DiscoverySource {
	/// The caller supplied the workspace.
	Explicit,
	/// Workspace discovery found the marker in the working directory or its ancestors.
	Ancestor,
}

/// Workspace discovery details and counts from its recorded tasks.
#[derive(Default, Serialize)]
pub struct InfoOutcome {
	/// Workspace root containing the Workyard marker.
	pub workspace: Option<NativePath>,
	/// Directory containing the task workspaces.
	pub tasks_root: Option<NativePath>,
	/// Directory holding authoritative Workyard state.
	pub state_dir: Option<NativePath>,
	/// How the workspace was selected.
	pub discovery: Option<DiscoverySource>,
	/// Number of active tasks observed.
	pub active_tasks: usize,
	/// Number of retired tasks observed.
	pub retired_tasks: usize,
	/// Number of records whose task facts could not be established.
	pub unresolved_records: usize,
	/// Number of tasks observed under operation admission.
	pub busy_tasks: usize,
	/// Actionable guidance associated with this result.
	pub guidance: Option<String>,
	/// Diagnostic describing a failure or an unresolved condition.
	pub problem: Option<ProblemOutcome>,
}
