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
	pub(super) fail_unlink: Option<UnlinkBoundary>,
}

#[derive(Clone, Copy)]
pub(super) enum UnlinkBoundary {
	Validate,
	Writer,
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
		before_unlink: impl FnOnce() -> Result<(), Problem> + Send,
	) -> Result<(), workyard_core::UnlinkFailure> {
		if let Some(boundary) = self.fail_unlink {
			let problem = Problem::new(
				ProblemCode::SupervisionFailed,
				"Injected supervision failure",
			);
			return match boundary {
				UnlinkBoundary::Validate => Err(UnlinkFailure::BeforeLaunch(problem)),
				UnlinkBoundary::Writer => {
					before_unlink()?;
					Err(UnlinkFailure::Command(problem))
				}
			};
		}

		self.runner
			.remove_worktree(observation, discard, control, before_unlink)
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

impl ObservationGit for FaultGit {
	async fn observe(
		&self,
		participant: &Participant,
		control: &Control,
	) -> Result<GitFacts, Problem> {
		self.runner.observe(participant, control).await
	}
}

impl RecoveryGit for FaultGit {
	async fn reconcile(
		&self,
		participant: &Participant,
		task: &str,
		control: &Control,
	) -> Result<RecoveryObservation, Problem> {
		self.runner.reconcile(participant, task, control).await
	}
}

impl AmendGit for FaultGit {
	async fn relationship(
		&self,
		repository: &Repository,
		participant: &Participant,
		control: &Control,
	) -> Result<Relationship, Problem> {
		self.runner
			.relationship(repository, participant, control)
			.await
	}
	async fn repair(
		&self,
		repository: &Repository,
		participant: &Participant,
		control: &Control,
	) -> Result<(), Problem> {
		self.runner.repair(repository, participant, control).await
	}
	async fn move_worktree(
		&self,
		repository: &Repository,
		participant: &Participant,
		destination: &Path,
		control: &Control,
	) -> Result<(), Problem> {
		self.runner
			.move_worktree(repository, participant, destination, control)
			.await
	}
}
