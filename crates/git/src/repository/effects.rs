use super::Repository;
use crate::{Runner, process::args};
use std::path::Path;
use workyard_core::BranchPlan;
use workyard_core::{Control, Problem};

impl Repository {
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
}
