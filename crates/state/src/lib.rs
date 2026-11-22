//! Versioned documents and stable OS locks for a single workspace.
//!
//! Use [`Reader`] for observations and [`Store`] for admitted changes. Records
//! describe task membership and progress. Separate evidence lets recovery
//! reconcile interrupted writes with the worktrees Git actually created.
#![deny(missing_docs)]

mod address;
mod damage;
mod discovery;
mod documents;
mod evidence;
pub mod path;
mod paths;
mod reader;
mod records;
mod revision;
mod store;
mod workspace;

pub use discovery::{discover_state, find_state};
pub use paths::{canonical_future, exists};
pub use reader::Reader;
pub use records::{OperationRecord, TaskRecord, task_filename};
pub use store::{Store, TaskGuard};
pub use workspace::{RootEdit, initialize_workspace, initialize_workspace_with_control};
pub use workyard_system::{Lock, LockMode};
