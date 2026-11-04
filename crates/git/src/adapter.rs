use crate::{Repository, Runner};
use std::path::Path;
use workyard_core::{BranchPlan, Control, Git, Problem, ProblemCode};

fn local(repository: &workyard_core::Repository) -> Repository {
	Repository {
		source: repository.source.clone(),
		common_dir: repository.common_dir.clone(),
		remote: repository.remote.clone(),
		identity: repository.identity.clone(),
	}
}

impl Git for Runner {
	async fn validate_branch(
		&self,
		root: &Path,
		task: &str,
		control: &Control,
	) -> Result<(), Problem> {
		let output = self
			.git(root, &["check-ref-format", "--branch", task], control)
			.await?;
		if !output.status.success() {
			return Err(Problem::new(
				ProblemCode::InvalidTask,
				"Task is not a valid Git branch name.",
			));
		}

		Ok(())
	}

	async fn inspect(
		&self,
		path: &Path,
		remote: Option<&str>,
		control: &Control,
	) -> Result<workyard_core::Repository, Problem> {
		let repository = Repository::inspect(self, path, remote, control).await?;
		Ok(workyard_core::Repository {
			source: repository.source,
			common_dir: repository.common_dir,
			remote: repository.remote,
			identity: repository.identity,
		})
	}

	async fn revalidate(
		&self,
		repository: &workyard_core::Repository,
		control: &Control,
	) -> Result<(), Problem> {
		local(repository).revalidate(self, control).await
	}

	async fn plan(
		&self,
		repository: &workyard_core::Repository,
		task: &str,
		checkout: bool,
		from: Option<&str>,
		resolution: workyard_core::SourceResolution,
		control: &Control,
	) -> Result<BranchPlan, Problem> {
		local(repository)
			.plan(self, task, checkout, from, resolution, control)
			.await
	}

	async fn fetch(
		&self,
		repository: &workyard_core::Repository,
		control: &Control,
	) -> Result<(), Problem> {
		let _lock = fetch_lock(repository, control).await?;
		self.revalidate(repository, control).await?;
		local(repository).fetch(self, control).await
	}

	async fn add(
		&self,
		repository: &workyard_core::Repository,
		task: &str,
		destination: &Path,
		plan: &BranchPlan,
		control: &Control,
	) -> Result<(), Problem> {
		local(repository)
			.add(self, task, destination, plan, control)
			.await
	}

	async fn pull(
		&self,
		repository: &workyard_core::Repository,
		destination: &Path,
		control: &Control,
	) -> Result<(), Problem> {
		let _lock = fetch_lock(repository, control).await?;
		self.revalidate(repository, control).await?;
		local(repository).pull(self, destination, control).await
	}
}

async fn fetch_lock(
	repository: &workyard_core::Repository,
	control: &Control,
) -> Result<workyard_system::Lock, Problem> {
	let directory = repository.common_dir.join("workyard");
	std::fs::create_dir_all(&directory)
		.map_err(|e| Problem::io("Create Git coordination directory", e))?;
	let lock = workyard_system::Lock::wait(&directory.join("fetch.lock"), control).await?;
	lock.verify()?;
	Ok(lock)
}
