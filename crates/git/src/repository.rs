mod planning;

use crate::{Runner, process::args};
use std::path::{Path, PathBuf};
use workyard_core::{BranchPlan, Control, Problem};

#[derive(Clone, Debug)]
pub struct Repository {
	pub source: PathBuf,
	pub common_dir: PathBuf,
	pub remote: Option<String>,
	pub identity: String,
}

impl Repository {
	pub async fn inspect(
		runner: &Runner,
		path: &Path,
		requested_remote: Option<&str>,
		control: &Control,
	) -> Result<Self, Problem> {
		let source = path
			.canonicalize()
			.map_err(|e| Problem::io(format!("Resolve repository {path:?}"), e))?;
		let common = runner
			.checked(
				&source,
				&args(&["rev-parse", "--path-format=absolute", "--git-common-dir"]),
				control,
			)
			.await?;
		let common_dir = decode_path(&common)?
			.canonicalize()
			.map_err(|e| Problem::io("Resolve Git common directory", e))?;
		let output = runner.checked(&source, &args(&["remote"]), control).await?;
		let remotes: Vec<_> = utf8(&output)?.lines().map(str::to_owned).collect();
		let remote = choose_remote(&remotes, requested_remote)?;
		let identity = workyard_system::directory_identity(&common_dir)?;
		Ok(Self {
			source,
			common_dir,
			remote,
			identity,
		})
	}

	pub async fn revalidate(&self, runner: &Runner, control: &Control) -> Result<(), Problem> {
		let current = Self::inspect(runner, &self.source, self.remote.as_deref(), control).await?;
		if current.common_dir != self.common_dir || current.identity != self.identity {
			return Err(Problem::new(
				workyard_core::ProblemCode::RepositoryChanged,
				format!("Git backing identity changed at {:?}.", self.source),
			));
		}

		Ok(())
	}

	pub async fn fetch(&self, runner: &Runner, control: &Control) -> Result<(), Problem> {
		if let Some(remote) = &self.remote {
			runner
				.effect(&self.source, &args(&["fetch", "--", remote]), control)
				.await?;
		}

		Ok(())
	}

	pub async fn add(
		&self,
		runner: &Runner,
		task: &str,
		destination: &Path,
		plan: &BranchPlan,
		control: &Control,
	) -> Result<(), Problem> {
		let mut arguments = args(&["worktree", "add"]);
		match plan {
			BranchPlan::Reuse => {
				arguments.push("--".into());
				arguments.push(destination.into());
				arguments.push(task.into());
			}
			BranchPlan::Track(source) => {
				arguments.extend(args(&["--track", "-b", task, "--"]));
				arguments.push(destination.into());
				arguments.push(source.into());
			}
			BranchPlan::Create(source) => {
				arguments.extend(args(&["-b", task, "--"]));
				arguments.push(destination.into());
				arguments.push(source.into());
			}
		}
		runner.effect(&self.source, &arguments, control).await?;
		Ok(())
	}

	pub async fn pull(
		&self,
		runner: &Runner,
		destination: &Path,
		control: &Control,
	) -> Result<(), Problem> {
		runner
			.effect(destination, &args(&["pull"]), control)
			.await?;
		Ok(())
	}
}

fn choose_remote(remotes: &[String], requested: Option<&str>) -> Result<Option<String>, Problem> {
	if let Some(requested) = requested {
		if !remotes.iter().any(|remote| remote == requested) {
			return Err(Problem::new(
				workyard_core::ProblemCode::InvalidRemote,
				format!("Remote {requested:?} is not configured."),
			));
		}
		return Ok(Some(requested.into()));
	}

	if remotes.iter().any(|remote| remote == "origin") {
		return Ok(Some("origin".into()));
	}
	match remotes {
		[] => Ok(None),
		[remote] => Ok(Some(remote.clone())),
		_ => Err(Problem::new(
			workyard_core::ProblemCode::RemoteRequired,
			"Several remotes are configured; supply --remote or --repo-remote.",
		)),
	}
}

fn utf8(bytes: &[u8]) -> Result<&str, Problem> {
	std::str::from_utf8(bytes).map_err(|_| {
		Problem::new(
			workyard_core::ProblemCode::InvalidGitOutput,
			"Git returned non-Unicode ref metadata.",
		)
	})
}

fn decode_path(bytes: &[u8]) -> Result<PathBuf, Problem> {
	crate::output::path_line(bytes).map_err(|_| {
		Problem::new(
			workyard_core::ProblemCode::InvalidGitOutput,
			"Git returned non-Unicode ref metadata.",
		)
	})
}

impl From<Repository> for workyard_core::Repository {
	fn from(repository: Repository) -> Self {
		Self {
			source: repository.source,
			common_dir: repository.common_dir,
			remote: repository.remote,
			identity: repository.identity,
		}
	}
}

impl From<&workyard_core::Repository> for Repository {
	fn from(repository: &workyard_core::Repository) -> Self {
		Self {
			source: repository.source.clone(),
			common_dir: repository.common_dir.clone(),
			remote: repository.remote.clone(),
			identity: repository.identity.clone(),
		}
	}
}
