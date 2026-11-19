//! Effect contracts grouped by the responsibility they describe.
mod amendment;
mod git;
mod observer;
mod removal;
mod state;

pub use amendment::{AmendGit, AmendState, Relationship};
pub use git::{BranchPlan, Git, RecoveryGit, Repository, SourceResolution};
pub use observer::Observer;
pub use state::{EffectState, RecoveryState, State};

pub use removal::{
	Decision, DecisionKind, RemovalGit, RemovalObservation, RemovalObserver, RemovalOptions,
	RetainedHistory, UnlinkFailure,
};
