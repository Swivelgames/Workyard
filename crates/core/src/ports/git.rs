use crate::{Control, Problem};
use std::{
	future::Future,
	path::{Path, PathBuf},
};

/// Identity, location and selected remote of a backing Git repository.
#[derive(Clone, Debug)]
pub struct Repository {
	/// Path to the backing repository.
	pub source: PathBuf,
	/// Git administration directory shared by the repository's worktrees.
	pub common_dir: PathBuf,
	/// Remote selected for branch resolution or fetch.
	pub remote: Option<String>,
	/// Stable identifier used to match repository observations.
	pub identity: String,
}

/// Git branch action to use when adding the linked worktree.
#[derive(Clone, Debug)]
pub enum BranchPlan {
	/// Check out the existing local branch.
	Reuse,
	/// Add the supplied roots to the project catalog configuration.
	Track(String),
	/// Create the local branch at the supplied starting revision.
	Create(String),
}

/// Which refs may be used during branch planning.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SourceResolution {
	/// Resolve using currently available refs.
	Cached,
	/// Plan before the requested fetch has completed.
	FetchPending,
}

/// Operations used to validate repository identity, resolve branches and establish worktrees.
pub trait Git: Sync {
	/// Identity used to verify the linked worktree before applying changes.
	fn worktree_identity(
		&self,
		_path: &Path,
	) -> impl Future<Output = Result<Option<String>, Problem>> + Send {
		async { Ok(None) }
	}

	/// Ask Git to validate the task's proposed branch name.
	fn validate_branch(
		&self,
		root: &Path,
		task: &str,
		control: &Control,
	) -> impl Future<Output = Result<(), Problem>> + Send;

	/// Resolve the backing repository, its identity and the selected remote.
	fn inspect(
		&self,
		path: &Path,
		remote: Option<&str>,
		control: &Control,
	) -> impl Future<Output = Result<Repository, Problem>> + Send;

	/// Check the repository against its earlier identity before proceeding.
	fn revalidate(
		&self,
		repository: &Repository,
		control: &Control,
	) -> impl Future<Output = Result<(), Problem>> + Send;

	/// Resolve reuse, tracking or creation of the task branch.
	fn plan(
		&self,
		repository: &Repository,
		task: &str,
		checkout: bool,
		from: Option<&str>,
		resolution: SourceResolution,
		control: &Control,
	) -> impl Future<Output = Result<BranchPlan, Problem>> + Send;

	/// Refresh refs from the selected remote under Workyard's fetch coordination.
	fn fetch(
		&self,
		repository: &Repository,
		control: &Control,
	) -> impl Future<Output = Result<(), Problem>> + Send;

	/// Create the linked worktree using the resolved branch plan.
	fn add(
		&self,
		repository: &Repository,
		task: &str,
		destination: &Path,
		plan: &BranchPlan,
		control: &Control,
	) -> impl Future<Output = Result<(), Problem>> + Send;
}

/// Git observations and repair operations available during task recovery.
pub trait RecoveryGit: Git {
	/// Discover an identity-verified location needing repair without changing Git.
	fn recovery_relationship(
		&self,
		_participant: &crate::Participant,
		_control: &Control,
	) -> impl Future<Output = Result<Option<crate::Relationship>, Problem>> + Send {
		async { Ok(None) }
	}

	/// Ask Git to reconcile verified backing and linked worktree paths.
	fn repair_relationship(
		&self,
		_repository: &Repository,
		_participant: &crate::Participant,
		_control: &Control,
	) -> impl Future<Output = Result<(), Problem>> + Send {
		async {
			Err(Problem::new(
				crate::ProblemCode::RepositoryChanged,
				"Git adapter does not support relationship repair.",
			))
		}
	}

	/// Inspect actual participant placement before resuming recorded work.
	fn reconcile(
		&self,
		participant: &crate::Participant,
		task: &str,
		control: &Control,
	) -> impl Future<Output = Result<crate::RecoveryObservation, Problem>> + Send;
}
