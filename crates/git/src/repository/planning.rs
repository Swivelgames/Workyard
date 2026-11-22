use super::{Repository, utf8};
use crate::{Runner, process::args};
use workyard_core::{BranchPlan, Control, Problem};

impl Repository {
	/// Resolve reuse, tracking or creation of the task branch.
	pub async fn plan(
		&self,
		runner: &Runner,
		task: &str,
		checkout: bool,
		from: Option<&str>,
		resolution: workyard_core::SourceResolution,
		control: &Control,
	) -> Result<BranchPlan, Problem> {
		let local = self
			.has_ref(runner, &format!("refs/heads/{task}"), control)
			.await?;
		let remote_ref = self
			.remote
			.as_ref()
			.map(|remote| format!("refs/remotes/{remote}/{task}"));
		let remote_exists = match &remote_ref {
			Some(reference) => self.has_ref(runner, reference, control).await?,
			None => false,
		};
		if !checkout && (local || remote_exists) {
			return Err(Problem::new(
				workyard_core::ProblemCode::BranchExists,
				format!(
					"Target branch {task:?} already exists in {:?}.",
					self.source
				),
			)
			.remedy(
				"Use `workyard checkout` or `workyard create --checkout` to permit branch reuse.",
			));
		}

		if local {
			self.ensure_not_checked_out(runner, task, control).await?;
			return Ok(BranchPlan::Reuse);
		}

		if remote_exists {
			return Ok(BranchPlan::Track(remote_ref.expect("checked remote")));
		}

		let (source, explicit_remote) = self.qualify_source(runner, from, control).await?;

		let reference = format!("{source}^{{commit}}");
		let resolved = runner
			.git(
				&self.source,
				&["rev-parse", "--verify", "--end-of-options", &reference],
				control,
			)
			.await?;
		if resolved.status.success() {
			return Ok(BranchPlan::Create(source));
		}

		// Only the selected remote can supply this missing source during the
		// pending fetch. Local refs and sources on other remotes remain errors.
		let selected_prefix = self
			.remote
			.as_ref()
			.map(|remote| format!("refs/remotes/{remote}/"));
		let can_fetch = resolution == workyard_core::SourceResolution::FetchPending
			&& explicit_remote
			&& selected_prefix.is_some_and(|prefix| source.starts_with(&prefix));
		if can_fetch {
			return Ok(BranchPlan::Create(source));
		}

		let message = format!(
			"Starting source {source:?} is unavailable in {:?}.",
			self.source
		);
		Err(Problem::new(workyard_core::ProblemCode::SourceMissing, message)
			.remedy("Supply an existing local --from/--repo-from branch, explicit commit, or explicit remote-qualified source such as origin/main."))
	}

	async fn qualify_source(
		&self,
		runner: &Runner,
		from: Option<&str>,
		control: &Control,
	) -> Result<(String, bool), Problem> {
		let source = match from {
			Some(source) => source.to_owned(),
			None => self.default_branch(runner, control).await?,
		};
		// Resolve unqualified branch names as local refs before Git's generic
		// revision parser. Explicit remote-qualified/commit inputs retain meaning.
		let remotes = runner
			.checked(&self.source, &args(&["remote"]), control)
			.await?;
		let explicit_remote = from.is_some()
			&& utf8(&remotes)?
				.lines()
				.any(|remote| source.starts_with(&format!("{remote}/")));
		let local_source = format!("refs/heads/{source}");
		let source = if explicit_remote {
			format!("refs/remotes/{source}")
		} else if self.has_ref(runner, &local_source, control).await? {
			local_source
		} else {
			source
		};

		Ok((source, explicit_remote))
	}

	async fn default_branch(&self, runner: &Runner, control: &Control) -> Result<String, Problem> {
		if let Some(remote) = &self.remote {
			let reference = format!("refs/remotes/{remote}/HEAD");
			let output = runner
				.git(
					&self.source,
					&["symbolic-ref", "--quiet", &reference],
					control,
				)
				.await?;
			if output.status.success() {
				let prefix = format!("refs/remotes/{remote}/");
				if let Some(branch) = utf8(&output.stdout)?.trim().strip_prefix(&prefix) {
					let local = format!("refs/heads/{branch}");
					if self.has_ref(runner, &local, control).await? {
						return Ok(local);
					}
				}
			}
		}
		let message = format!(
			"No usable local repository-default branch metadata in {:?}.",
			self.source
		);
		Err(Problem::new(workyard_core::ProblemCode::DefaultBranchUnknown, message)
			.remedy("Supply --from or --repo-from explicitly; Workyard does not guess from the current checkout or main/master."))
	}

	async fn has_ref(
		&self,
		runner: &Runner,
		reference: &str,
		control: &Control,
	) -> Result<bool, Problem> {
		let output = runner
			.git(
				&self.source,
				&["show-ref", "--verify", "--quiet", reference],
				control,
			)
			.await?;
		match output.status.code() {
			Some(0) => Ok(true),
			Some(1) => Ok(false),
			_ => Err(output.problem(
				&self.source,
				&args(&["show-ref", "--verify", "--quiet", reference]),
			)),
		}
	}

	async fn ensure_not_checked_out(
		&self,
		runner: &Runner,
		task: &str,
		control: &Control,
	) -> Result<(), Problem> {
		let output = runner
			.checked(
				&self.source,
				&args(&["worktree", "list", "--porcelain", "-z"]),
				control,
			)
			.await?;
		let target = format!("branch refs/heads/{task}");
		if output
			.split(|byte| *byte == 0)
			.any(|field| field == target.as_bytes())
		{
			return Err(Problem::new(
				workyard_core::ProblemCode::BranchInUse,
				format!(
					"Branch {task:?} is already checked out in {:?}.",
					self.source
				),
			)
			.remedy(format!(
				"Inspect `git -C {:?} worktree list --porcelain`; choose a free branch.",
				self.source
			)));
		}

		Ok(())
	}
}
