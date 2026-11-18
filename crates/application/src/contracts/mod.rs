//! Shared vocabulary and pure core-to-contract mappings; no IO or interface policy.
mod condition;
mod operation;
mod participant;
mod problem;
mod progress;
mod pull;
pub(crate) mod relocation;
pub(crate) mod removal;
mod repository;

pub use condition::Condition;
pub use operation::OperationReport;
pub use participant::ParticipantOutcome;
pub use problem::ProblemOutcome;
pub use progress::Progress;
pub use pull::PullFailure;
pub use repository::RepositoryInput;
