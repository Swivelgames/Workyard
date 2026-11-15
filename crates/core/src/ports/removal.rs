use crate::{Control, Git, Participant, Problem, Repository};
use std::{future::Future, path::PathBuf};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RetainedHistory {
	UpToDate,
	Unpushed,
	NoUpstream,
	Detached,
}

#[derive(Clone, Debug)]
pub struct RemovalObservation {
	pub repository: Repository,
	pub destination: Option<PathBuf>,
	pub branch: Option<String>,
	pub dirty_paths: Vec<PathBuf>,
	/// Content-sensitive token scoped to this inspection, never durable consent.
	pub dirty_token: Vec<u8>,
	pub history: RetainedHistory,
}

pub trait RemovalGit: Git {
	/// Only for durable successful recreation evidence, never a failed add attempt.
	fn reconcile_recreation(
		&self,
		participant: &Participant,
		branch: &str,
		control: &Control,
	) -> impl Future<Output = Result<crate::RecoveryObservation, Problem>> + Send;

	fn reconcile_restored(
		&self,
		participant: &Participant,
		control: &Control,
	) -> impl Future<Output = Result<crate::RecoveryObservation, Problem>> + Send;

	fn inspect_removal(
		&self,
		participant: &Participant,
		control: &Control,
	) -> impl Future<Output = Result<RemovalObservation, Problem>> + Send;
	fn remove_worktree(
		&self,
		observation: &RemovalObservation,
		discard: bool,
		control: &Control,
	) -> impl Future<Output = Result<(), Problem>> + Send;
	fn inspect_restoration(
		&self,
		participant: &Participant,
		branch: &str,
		control: &Control,
	) -> impl Future<Output = Result<Repository, Problem>> + Send;
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DecisionKind {
	RemoveTask,
	DiscardTask,
	DiscardFiles,
	RetainedHistory,
	LostChanges,
}

#[derive(Clone, Debug)]
pub struct Decision {
	pub task: Option<String>,
	pub kind: DecisionKind,
	pub participant: String,
	pub message: String,
}

pub trait RemovalObserver: crate::Observer {
	fn disclose(
		&self,
		participant: &str,
		observation: &RemovalObservation,
	) -> impl Future<Output = Result<(), Problem>> + Send;
	/// Returns None when the interface cannot ask; never silently grants consent.
	fn confirm(
		&self,
		decision: Decision,
	) -> impl Future<Output = Result<Option<bool>, Problem>> + Send;
}

#[derive(Clone, Copy, Debug, Default)]
pub struct RemovalOptions {
	pub yes: bool,
	pub force: bool,
	pub undo: bool,
}
