//! Effect contracts grouped by the responsibility they describe.
mod git;
mod observer;
mod state;

pub use git::{BranchPlan, Git, RecoveryGit, Repository, SourceResolution};
pub use observer::Observer;
pub use state::{EffectState, RecoveryState, State};
