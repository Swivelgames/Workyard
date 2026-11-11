use crate::{Control, Problem};
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

pub trait RecoveryGit: Git {
	fn reconcile(
		&self,
		participant: &crate::Participant,
		task: &str,
		control: &Control,
	) -> impl Future<Output = Result<crate::RecoveryObservation, Problem>> + Send;
}
