//! Versioned documents and stable OS locks for a single workspace.
mod documents;
pub mod path;
mod paths;
mod records;
mod store;

pub use paths::{canonical_future, exists};
pub use records::{TaskRecord, task_filename};
pub use store::{Store, TaskGuard};
pub use workyard_system::Lock;
