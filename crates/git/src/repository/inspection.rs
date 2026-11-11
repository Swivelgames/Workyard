use super::Repository;
use super::remote::choose_remote;
use super::{decode_path, utf8};
use crate::{Runner, process::args};
use std::path::Path;
use workyard_core::{Control, Problem};

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
}
