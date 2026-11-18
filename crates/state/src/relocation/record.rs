use crate::{TaskRecord, path::StoredPath};
use serde::{Deserialize, Serialize};

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Participant {
	pub task: TaskRecord,
	pub index: usize,
	pub destination: StoredPath,
	#[serde(default)]
	pub absent_at_admission: bool,
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub selection: Option<super::selection::Selection>,
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub recovered_source: Option<(u64, u64)>,
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub recovered_destination: Option<(u64, u64)>,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct StateFile {
	pub path: StoredPath,
	pub original: Vec<u8>,
	pub target: Vec<u8>,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Content {
	pub path: StoredPath,
	pub kind: String,
	pub digest: String,
	pub link: Option<StoredPath>,
	pub copied: bool,
	pub origin_identity: Option<(u64, u64)>,
	pub destination_identity: Option<(u64, u64)>,
	pub staged_identity: Option<(u64, u64)>,
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub staged_for_undo: Option<bool>,
	pub mode: Option<u32>,
	pub readonly: bool,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Journal {
	pub version: u32,
	pub id: String,
	#[serde(default)]
	pub copy: bool,
	#[serde(default)]
	pub copy_verified: bool,
	pub old_root: StoredPath,
	pub new_root: StoredPath,
	pub old_state: StoredPath,
	pub new_state: StoredPath,
	pub participants: Vec<Participant>,
	pub files: Vec<StateFile>,
	pub state_directories: Vec<StoredPath>,
	pub content: Vec<Content>,
	pub writer_pending: bool,
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub writer_description: Option<String>,
	pub activated: bool,
	pub activation_pending: bool,
	pub undone: bool,
	pub cleanup_complete: bool,
}
