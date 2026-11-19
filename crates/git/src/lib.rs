//! Installed Git is authoritative for repository and worktree relationships.
mod adapter;
pub mod completion;
mod observation;
mod operations;
mod output;
mod process;
pub mod projects;
mod recovery;
mod relationships;
mod removal;
mod repository;
#[cfg(unix)]
mod unix;
#[cfg(windows)]
mod windows;

pub use process::{CommandOutput, Runner};
pub use relationships::Placement;
pub use repository::Repository;
