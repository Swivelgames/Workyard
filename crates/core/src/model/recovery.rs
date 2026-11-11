use crate::{Repository, Task};
use std::path::PathBuf;

#[derive(Clone, Debug)]
pub struct RecoveryRecord {
	pub task: Task,
	pub pending: Option<usize>,
}

#[derive(Clone, Debug)]
pub struct RecoveryObservation {
	pub repository: Repository,
	pub destination: Option<PathBuf>,
	pub operation_in_progress: bool,
}
