//! Shared vocabulary and pure core-to-contract mappings; no IO or interface policy.
mod condition;
mod operation;
mod participant;
mod problem;
mod progress;
pub(crate) mod removal;
mod repository;

pub use condition::Condition;
pub use operation::OperationReport;
pub use participant::{ParticipantChange, ParticipantOutcome};
pub use problem::ProblemOutcome;
pub use progress::Progress;
pub use repository::RepositoryInput;
