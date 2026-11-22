//! Installed Git is authoritative for repository and worktree relationships.
//!
//! [`Runner`] supervises the installed Git executable. Branch planning, relationship
//! inspection and repair use repository identity to verify the intended worktree.
#![deny(missing_docs)]

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
mod worktree_path;

pub use process::{CommandOutput, Runner};
pub use relationships::Placement;
pub use repository::Repository;
