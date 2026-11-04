//! Installed Git is authoritative for repository and worktree relationships.
mod adapter;
mod process;
mod repository;
#[cfg(unix)]
mod unix;
#[cfg(windows)]
mod windows;

pub use process::{CommandOutput, Runner};
pub use repository::Repository;
