//! Persisted documents grouped by their aggregate owner.
pub(crate) mod registrations;
pub(crate) mod task;

pub use crate::address::task_filename;
pub use task::{OperationRecord, ProblemRecord, TaskRecord};
