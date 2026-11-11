//! Persisted documents grouped by their aggregate owner.
pub(crate) mod task;
mod workspace;

pub use crate::address::task_filename;
pub use task::{OperationRecord, ProblemRecord, TaskRecord};
pub use workspace::WorkspaceRecord;
