//! Stable native file identity and cooperative OS locks shared by adapters.
mod identity;
mod lock;
mod movement;

pub use identity::{directory_identity, entry_identity, file_identity};
pub use lock::{Lock, LockMode};
pub use movement::rename_no_replace;
