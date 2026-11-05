//! Versioned documents and stable OS locks for a single workspace.
mod damage;
mod discovery;
mod documents;
mod evidence;
pub mod path;
mod paths;
mod records;
mod recovery_record;
mod store;

pub use discovery::discover_state;
pub use paths::{canonical_future, exists};
pub use records::{TaskRecord, task_filename};
pub use store::{Store, TaskGuard};
pub use workyard_system::{Lock, LockMode};
