//! Stable native file identity and cooperative OS locks shared by adapters.
//!
//! [`Lock`] holds a stable file across state replacements. Native file identity
//! lets a caller detect when a named path has been replaced.
#![deny(missing_docs)]

mod identity;
mod lock;

pub use identity::{directory_identity, entry_identity, file_identity};
pub use lock::{Lock, LockMode};
