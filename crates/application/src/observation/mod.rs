//! Shared observation facts and execution; capabilities own entries and views.
mod condition;
mod contract;
pub use condition::{Condition, Provenance};
mod execution;
mod guidance;
pub(crate) mod render;

pub use contract::{Observation, ParticipantReport, ParticipantState, SavedTask, TaskReport};
pub(crate) use execution::{Detail, read};
