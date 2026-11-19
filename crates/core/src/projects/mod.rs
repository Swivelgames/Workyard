//! In-memory references derived from the currently discoverable Git repositories.
mod catalog;
pub(crate) mod names;

pub use catalog::{Catalog, Project, ProjectLocation};
