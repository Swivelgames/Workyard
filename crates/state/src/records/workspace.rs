use crate::path::StoredPath;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorkspaceRecord {
	pub version: u32,
	pub tasks_root: StoredPath,
	pub state_dir: StoredPath,
}
