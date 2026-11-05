//! Narrow effects required by create/checkout, independent of runtime and storage format.
use crate::{Control, Problem, Task};
use std::{
	future::Future,
	path::{Path, PathBuf},
};

#[derive(Clone, Debug)]
pub struct Repository {
	pub source: PathBuf,
	pub common_dir: PathBuf,
	pub remote: Option<String>,
	pub identity: String,
}

#[derive(Clone, Debug)]
pub enum BranchPlan {
	Reuse,
	Track(String),
	Create(String),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SourceResolution {
	Cached,
	FetchPending,
}

pub trait Git: Sync {
	fn worktree_identity(
		&self,
		_path: &Path,
	) -> impl Future<Output = Result<Option<String>, Problem>> + Send {
		async { Ok(None) }
	}

	fn validate_branch(
		&self,
		root: &Path,
		task: &str,
		control: &Control,
	) -> impl Future<Output = Result<(), Problem>> + Send;
	fn inspect(
		&self,
		path: &Path,
		remote: Option<&str>,
		control: &Control,
	) -> impl Future<Output = Result<Repository, Problem>> + Send;
	fn revalidate(
		&self,
		repository: &Repository,
		control: &Control,
	) -> impl Future<Output = Result<(), Problem>> + Send;
	fn plan(
		&self,
		repository: &Repository,
		task: &str,
		checkout: bool,
		from: Option<&str>,
		resolution: SourceResolution,
		control: &Control,
	) -> impl Future<Output = Result<BranchPlan, Problem>> + Send;
	fn fetch(
		&self,
		repository: &Repository,
		control: &Control,
	) -> impl Future<Output = Result<(), Problem>> + Send;
	fn add(
		&self,
		repository: &Repository,
		task: &str,
		destination: &Path,
		plan: &BranchPlan,
		control: &Control,
	) -> impl Future<Output = Result<(), Problem>> + Send;
	fn pull(
		&self,
		repository: &Repository,
		destination: &Path,
		control: &Control,
	) -> impl Future<Output = Result<(), Problem>> + Send;
}

/// Whether managed writers for the recorded effect have stopped.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EffectState {
	Pending,
	Settled,
}

pub trait State: Sync {
	type Admission: Send + Sync;
	fn verify(&self) -> Result<(), Problem>;
	fn validate_destination(&self, path: &Path) -> Result<(), Problem>;
	fn reserve(&self, path: &Path) -> Result<(), Problem>;
	fn admit(&self, name: &str) -> Result<Self::Admission, Problem>;
	fn ensure_new(&self, admission: &Self::Admission, name: &str) -> Result<(), Problem>;
	fn record_path(&self, admission: &Self::Admission) -> PathBuf;
	fn begin(&self, admission: &Self::Admission, task: &Task) -> Result<(), Problem>;
	fn save(&self, admission: &Self::Admission, task: &Task) -> Result<(), Problem>;
	/// Named entry point; delegates to the legacy hook for existing adapters.
	fn record_effect(
		&self,
		admission: &Self::Admission,
		task: &Task,
		index: usize,
		state: EffectState,
	) -> Result<(), Problem> {
		self.effect(admission, task, index, state == EffectState::Settled)
	}

	/// Compatibility hook for existing state adapters.
	fn effect(
		&self,
		_admission: &Self::Admission,
		_task: &Task,
		_index: usize,
		_settled: bool,
	) -> Result<(), Problem> {
		Ok(())
	}
}

pub trait RecoveryState: State {
	fn load(
		&self,
		admission: &Self::Admission,
		name: &str,
	) -> Result<crate::RecoveryRecord, Problem>;
}

pub trait RecoveryGit: Git {
	fn reconcile(
		&self,
		participant: &crate::Participant,
		task: &str,
		control: &Control,
	) -> impl Future<Output = Result<crate::RecoveryObservation, Problem>> + Send;
}

pub trait Observer: Sync {
	fn phase(
		&self,
		task: &str,
		participant: &str,
		phase: &str,
	) -> impl Future<Output = Result<(), Problem>> + Send;
}
