use crate::{Repository, Runner};
use std::path::Path;
use workyard_core::{BranchPlan, Control, Git, Problem, ProblemCode};

impl Git for Runner {
	async fn worktree_identity(&self, path: &Path) -> Result<Option<String>, Problem> {
		let output = self
			.checked(
				path,
				&crate::process::args(&["rev-parse", "--path-format=absolute", "--git-dir"]),
				&Control::new(|| false),
			)
			.await?;
		let directory = crate::output::path_line(&output).map_err(|_| {
			Problem::new(ProblemCode::RepositoryChanged, "Invalid Git path output.").remedy("Inspect git worktree list --porcelain and git status at the recorded paths. Repair Git relationships manually; preserve worktrees and native Git locks.")
		})?;
		Ok(Some(workyard_system::directory_identity(&directory)?))
	}

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
		Ok(repository.into())
	}

	async fn revalidate(
		&self,
		repository: &workyard_core::Repository,
		control: &Control,
	) -> Result<(), Problem> {
		Repository::from(repository).revalidate(self, control).await
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
		Repository::from(repository)
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
		Repository::from(repository).fetch(self, control).await
	}

	async fn add(
		&self,
		repository: &workyard_core::Repository,
		task: &str,
		destination: &Path,
		plan: &BranchPlan,
		control: &Control,
	) -> Result<(), Problem> {
		Repository::from(repository)
			.add(self, task, destination, plan, control)
			.await
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
