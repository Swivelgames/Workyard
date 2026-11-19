//! Workyard orchestration and interface DTOs; Git and state remain focused adapters.
mod actions;
mod application;
mod contracts;
mod execution;
mod guidance;
pub mod observation;
mod presentation;
mod projects;
mod workspace;

// Deliberate compatibility exports: disk ownership does not change supported imports.
pub use actions::{amend, create, info, init, list, recover, remove, status};
pub use application::application;
pub use contracts::{ProblemOutcome, Progress};
pub use create::{CreateInput, CreateOutcome, RepositoryInput, create};
