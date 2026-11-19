//! Stable native file identity and cooperative OS locks shared by adapters.
mod identity;
mod lock;

pub use identity::{directory_identity, entry_identity, file_identity};
pub use lock::{Lock, LockMode};
