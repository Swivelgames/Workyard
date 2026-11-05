//! Durable effect evidence shared by creation and recovery.
use crate::TaskRecord;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct EffectRecord {
	pub version: u32,
	pub task: TaskRecord,
	pub pending: Option<usize>,
}
