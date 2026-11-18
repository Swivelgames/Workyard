//! Versioned documents and stable OS locks for a single workspace.
mod address;
mod damage;
mod discovery;
mod documents;
mod evidence;
pub mod path;
mod paths;
mod reader;
mod records;
mod relocation;
mod revision;
mod store;

pub use discovery::{discover_state, find_state};
pub use paths::{canonical_future, exists};
pub use reader::Reader;
pub use records::{OperationRecord, TaskRecord, task_filename};
pub use relocation::{
	Relocation, RelocationStatus, SnapshotStatus, relocation_status, snapshot_status,
};
pub use store::{Store, TaskGuard};
pub use workyard_system::{Lock, LockMode};
