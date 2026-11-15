//! Real Git adapter with deterministic add/identity failure boundaries.
use std::{
	path::Path,
	sync::atomic::{AtomicBool, AtomicUsize, Ordering},
};
use workyard_core::*;

#[derive(Default)]
pub(super) struct FaultGit {
	pub(super) runner: workyard_git::Runner,
	pub(super) identity_failure: AtomicBool,
	pub(super) add_failure: bool,
	pub(super) adds: AtomicUsize,
}

impl Git for FaultGit {
	async fn worktree_identity(&self, path: &Path) -> Result<Option<String>, Problem> {
		if self.identity_failure.swap(false, Ordering::SeqCst) {
			return Err(Problem::new(
				ProblemCode::IoError,
				"Injected post-add identity failure",
			));
		}
		self.runner.worktree_identity(path).await
	}

	async fn validate_branch(
		&self,
		root: &Path,
		task: &str,
		control: &Control,
	) -> Result<(), Problem> {
		self.runner.validate_branch(root, task, control).await
	}

	async fn inspect(
		&self,
		path: &Path,
		remote: Option<&str>,
		control: &Control,
	) -> Result<Repository, Problem> {
		self.runner.inspect(path, remote, control).await
	}

	async fn revalidate(&self, repository: &Repository, control: &Control) -> Result<(), Problem> {
		self.runner.revalidate(repository, control).await
	}

	async fn plan(
		&self,
		repository: &Repository,
		task: &str,
		checkout: bool,
		from: Option<&str>,
		resolution: SourceResolution,
		control: &Control,
	) -> Result<BranchPlan, Problem> {
		self.runner
			.plan(repository, task, checkout, from, resolution, control)
			.await
	}

	async fn fetch(&self, repository: &Repository, control: &Control) -> Result<(), Problem> {
		self.runner.fetch(repository, control).await
	}

	async fn add(
		&self,
		repository: &Repository,
		task: &str,
		destination: &Path,
		plan: &BranchPlan,
		control: &Control,
	) -> Result<(), Problem> {
		self.adds.fetch_add(1, Ordering::SeqCst);
		if self.add_failure {
			return Err(Problem::new(ProblemCode::GitFailed, "Injected failed add"));
		}
		self.runner
			.add(repository, task, destination, plan, control)
			.await
	}

	async fn pull(
		&self,
		repository: &Repository,
		destination: &Path,
		control: &Control,
	) -> Result<(), Problem> {
		self.runner.pull(repository, destination, control).await
	}
}

impl RemovalGit for FaultGit {
	async fn reconcile_recreation(
		&self,
		participant: &Participant,
		branch: &str,
		control: &Control,
	) -> Result<RecoveryObservation, Problem> {
		self.runner
			.reconcile_recreation(participant, branch, control)
			.await
	}

	async fn reconcile_restored(
		&self,
		participant: &Participant,
		control: &Control,
	) -> Result<RecoveryObservation, Problem> {
		self.runner.reconcile_restored(participant, control).await
	}

	async fn inspect_removal(
		&self,
		participant: &Participant,
		control: &Control,
	) -> Result<RemovalObservation, Problem> {
		self.runner.inspect_removal(participant, control).await
	}

	async fn remove_worktree(
		&self,
		observation: &RemovalObservation,
		discard: bool,
		control: &Control,
	) -> Result<(), Problem> {
		self.runner
			.remove_worktree(observation, discard, control)
			.await
	}

	async fn inspect_restoration(
		&self,
		participant: &Participant,
		branch: &str,
		control: &Control,
	) -> Result<Repository, Problem> {
		self.runner
			.inspect_restoration(participant, branch, control)
			.await
	}
}
