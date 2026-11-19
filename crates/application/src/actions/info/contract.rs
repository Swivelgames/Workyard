use crate::contracts::ProblemOutcome;
use serde::Serialize;
use trine::NativePath;

#[derive(Clone, Copy, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DiscoverySource {
	Explicit,
	Ancestor,
}

#[derive(Default, Serialize)]
pub struct InfoOutcome {
	pub workspace: Option<NativePath>,
	pub tasks_root: Option<NativePath>,
	pub state_dir: Option<NativePath>,
	pub discovery: Option<DiscoverySource>,
	pub active_tasks: usize,
	pub retired_tasks: usize,
	pub unresolved_records: usize,
	pub busy_tasks: usize,
	pub guidance: Option<String>,
	pub problem: Option<ProblemOutcome>,
}
