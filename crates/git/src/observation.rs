//! Git-current inspection without index refresh or lifecycle adoption decisions.
use crate::{Runner, output::path_line, process::args};
use std::path::{Path, PathBuf};
use workyard_core::{Control, GitFacts, ObservationGit, Participant, Phase, Problem, ProblemCode};

struct RegisteredWorktree {
	path: PathBuf,
	git_dir: PathBuf,
	common: PathBuf,
	identity: String,
}

struct Observation<'a> {
	runner: &'a Runner,
}

impl ObservationGit for Runner {
	async fn observe(
		&self,
		participant: &Participant,
		control: &Control,
	) -> Result<GitFacts, Problem> {
		Observation { runner: self }
			.observe(participant, control)
			.await
	}
}

impl Observation<'_> {
	async fn observe(
		&self,
		participant: &Participant,
		control: &Control,
	) -> Result<GitFacts, Problem> {
		let repository = self.locate_backing(participant, control).await?;
		let current = self
			.find_participant(&repository, participant, control)
			.await?;
		let Some(current) = current else {
			return missing_participant(participant);
		};
		self.inspect_worktree(current, control).await
	}

	async fn find_participant(
		&self,
		repository: &Path,
		participant: &Participant,
		control: &Control,
	) -> Result<Option<RegisteredWorktree>, Problem> {
		let output = self
			.runner
			.checked(
				repository,
				&args(&["worktree", "list", "--porcelain", "-z"]),
				control,
			)
			.await?;
		let mut current = None;
		for field in output.split(|byte| *byte == 0) {
			control.check()?;
			let Some(bytes) = field.strip_prefix(b"worktree ") else {
				continue;
			};
			let path = crate::output::path_field(bytes)
				.map_err(|_| changed("Invalid Git worktree path."))?;
			// Unrelated prunable or inaccessible registrations cannot establish this
			// participant's identity and must not hide a reachable matching worktree.
			match path.try_exists() {
				Ok(true) => {}
				Err(error) if path == participant.destination => {
					return Err(Problem::io("Inspect registered worktree location", error));
				}
				Ok(false) | Err(_) => continue,
			}
			let candidate = match self.registered_worktree(&path, control).await {
				Ok(candidate) => candidate,
				Err(problem) if problem.code == ProblemCode::Cancelled => return Err(problem),
				Err(problem) if path == participant.destination => return Err(problem),
				Err(_) => continue,
			};
			let matches = participant
				.worktree_identity
				.as_ref()
				.map_or(path == participant.destination, |expected| {
					expected == &candidate.identity
				});
			if !matches {
				continue;
			}
			if workyard_system::directory_identity(&candidate.common)?
				!= participant.backing_identity
			{
				return Err(changed("Registered participant backing identity differs."));
			}
			if current.replace(candidate).is_some() {
				return Err(changed(
					"Several registrations match the participant identity.",
				));
			}
		}
		Ok(current)
	}

	async fn registered_worktree(
		&self,
		path: &Path,
		control: &Control,
	) -> Result<RegisteredWorktree, Problem> {
		let git_dir = self.git_directory(path, "--git-dir", control).await?;
		let identity = workyard_system::directory_identity(&git_dir)?;
		Ok(RegisteredWorktree {
			path: path.into(),
			git_dir,
			identity,
			common: self
				.git_directory(path, "--git-common-dir", control)
				.await?,
		})
	}

	async fn inspect_worktree(
		&self,
		current: RegisteredWorktree,
		control: &Control,
	) -> Result<GitFacts, Problem> {
		let branch = self
			.runner
			.checked(
				&current.path,
				&args(&["rev-parse", "--abbrev-ref", "HEAD"]),
				control,
			)
			.await?;
		let branch = std::str::from_utf8(&branch)
			.map_err(|_| changed("Invalid Git branch output."))?
			.trim_end_matches(['\r', '\n'])
			.to_owned();
		let status = self
			.runner
			.checked(
				&current.path,
				&args(&[
					"--no-optional-locks",
					"status",
					"--porcelain",
					"-z",
					"--untracked-files=normal",
				]),
				control,
			)
			.await?;
		let conflicts = self
			.runner
			.checked(
				&current.path,
				&args(&["ls-files", "--unmerged", "-z"]),
				control,
			)
			.await?;
		let operation_in_progress =
			crate::operations::operations_present(&current.git_dir, &current.common)?;
		Ok(GitFacts {
			common_dir: Some(current.common),
			problem: None,
			path: Some(current.path),
			branch: Some(branch),
			dirty: !status.is_empty(),
			conflicted: !conflicts.is_empty(),
			operation_in_progress,
		})
	}

	async fn git_directory(
		&self,
		path: &Path,
		selector: &str,
		control: &Control,
	) -> Result<PathBuf, Problem> {
		let output = self
			.runner
			.checked(
				path,
				&args(&["rev-parse", "--path-format=absolute", selector]),
				control,
			)
			.await?;
		path_line(&output).map_err(|_| changed("Invalid Git directory output."))
	}

	async fn locate_backing(
		&self,
		participant: &Participant,
		control: &Control,
	) -> Result<PathBuf, Problem> {
		for path in [
			&participant.source,
			&participant.destination,
			&participant.common_dir,
		] {
			if !path
				.try_exists()
				.map_err(|e| Problem::io("Inspect cached Git location", e))?
			{
				continue;
			}
			let common = self
				.git_directory(path, "--git-common-dir", control)
				.await?;
			if workyard_system::directory_identity(&common)? != participant.backing_identity {
				return Err(changed(
					"Cached Git location now refers to different backing storage.",
				));
			}
			return Ok(path.clone());
		}
		Err(changed("Backing Git storage is unreachable."))
	}
}

fn missing_participant(participant: &Participant) -> Result<GitFacts, Problem> {
	if participant
		.destination
		.try_exists()
		.map_err(|error| Problem::io("Inspect recorded destination", error))?
	{
		return Err(changed(
			"Recorded destination exists but its participant identity is not registered.",
		));
	}
	let problem = if matches!(
		participant.phase,
		Phase::WorktreeCreated | Phase::Pulling | Phase::Completed
	) {
		Some(changed(
			"Previously created participant worktree is missing. Restore access or repair its Git registration before recovery.",
		))
	} else {
		None
	};
	Ok(GitFacts {
		common_dir: None,
		problem,
		path: None,
		branch: None,
		dirty: false,
		conflicted: false,
		operation_in_progress: false,
	})
}

fn changed(message: &str) -> Problem {
	Problem::new(ProblemCode::RepositoryChanged, message).remedy("Inspect git worktree list --porcelain and git status. Repair moved Git relationships with git worktree repair from the main clone; preserve worktrees and state.")
}
