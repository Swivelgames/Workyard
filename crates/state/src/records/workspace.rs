use crate::path::StoredPath;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorkspaceRecord {
	pub version: u32,
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub relocation: Option<String>,
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub authority: Option<String>,
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub relocation_source: Option<StoredPath>,
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub relocation_destination: Option<StoredPath>,
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub active_tasks_root: Option<StoredPath>,
	pub tasks_root: StoredPath,
	pub state_dir: StoredPath,
}

impl WorkspaceRecord {
	pub(crate) fn read(path: &std::path::Path) -> Result<Self, workyard_core::Problem> {
		crate::documents::read_supported(path, &[1, 2, 3])
	}
}
