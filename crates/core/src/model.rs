//! Cohesive domain vocabularies, re-exported through the core facade.
mod amendment;
mod creation;
mod operation;
mod recovery;
mod removal;
mod task;

pub use amendment::{AmendRequest, AmendResult, AmendmentChange, AmendmentChangeKind};
pub use creation::{CreateRequest, CreateResult, CreationOptions, PullFailure, RepositoryInput};
pub use recovery::{RecoveryObservation, RecoveryRecord, RecoveryResult};
pub use task::{Participant, Phase, Task, TaskLifecycle};

pub use operation::{Operation, OperationKind, OperationOutcome};

pub use removal::{Removal, RemovalDirection, RemovalParticipant, RemovalPhase, RemovalResult};
