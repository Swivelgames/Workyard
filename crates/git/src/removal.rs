//! Destructive worktree operations verify Git ownership independently of cached paths.
mod dirty;
mod protected;
mod restoration;
mod validation;

use crate::{Repository, Runner, process::args};
use workyard_core::{
	Control, ObservationGit, Participant, Problem, ProblemCode, RemovalGit, RemovalObservation,
	RetainedHistory,
};

impl RemovalGit for Runner {
	async fn inspect_removal(
		&self,
		participant: &Participant,
		control: &Control,
	) -> Result<RemovalObservation, Problem> {
		let facts = self.observe(participant, control).await?;
		let Some(destination) = facts.path else {
			return Err(blocked(
				"Managed worktree is missing; repair its registration before removal.",
			));
		};
		if destination != participant.destination {
			match std::fs::symlink_metadata(&participant.destination) {
				Ok(_) => {
					return Err(blocked(
						"The recorded participant location is occupied by other files. Inspect both locations before removing the worktree.",
					));
				}
				Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
				Err(error) => return Err(Problem::io("Inspect recorded worktree location", error)),
			}
		}
		let worktree_entry = workyard_system::entry_identity(&destination)?;
		let repository = Repository::local(self, &destination, control).await?;
		let output = self
			.checked(
				&destination,
				&args(&["rev-parse", "--path-format=absolute", "--git-dir"]),
				control,
			)
			.await?;
		let git_dir = crate::output::path_line(&output)
			.map_err(|_| blocked("Invalid Git administrative path."))?;
		if git_dir == repository.common_dir {
			return Err(blocked(
				"Main worktrees cannot be removed as task participants.",
			));
		}
		if facts.conflicted {
			return Err(blocked("Worktree has unresolved Git conflicts."));
		}
		validation::require_removable_admin(&git_dir, &repository.common_dir)?;

		let branch = facts.branch.filter(|branch| branch != "HEAD");
		let history = match &branch {
			Some(branch) => self.retained_history(&destination, branch, control).await?,
			None => {
				self.require_retained_head(&destination, control).await?;
				RetainedHistory::Detached
			}
		};
		let (dirty_paths, dirty_token) =
			dirty::inspect(self, &destination, worktree_entry, control).await?;
		Ok(RemovalObservation {
			administration_identity: workyard_system::directory_identity(&git_dir)?,
			worktree_entry,
			repository: repository.into(),
			destination: Some(destination),
			branch,
			dirty_paths,
			dirty_token,
			history,
		})
	}

	async fn remove_worktree(
		&self,
		observation: &RemovalObservation,
		discard: bool,
		control: &Control,
		before_unlink: impl FnOnce() -> Result<(), Problem> + Send,
	) -> Result<(), workyard_core::UnlinkFailure> {
		let destination = observation
			.destination
			.as_ref()
			.ok_or_else(|| blocked("Removal has no verified worktree destination."))?;

		let _gate =
			crate::relationships::relationship_lock(&observation.repository, control).await?;
		let admin = self
			.verify_removal_location(observation, destination, control)
			.await?;
		self.validate_disclosure(observation, destination, &admin, control)
			.await?;

		let mut arguments = args(&["worktree", "remove"]);
		if discard {
			arguments.push("--force".into());
		}
		arguments.push("--".into());
		arguments.push(dunce::simplified(destination).into());

		before_unlink()?;
		self.effect(&observation.repository.common_dir, &arguments, control)
			.await
			.map_err(workyard_core::UnlinkFailure::Command)
	}

	async fn inspect_restoration(
		&self,
		participant: &Participant,
		branch: &str,
		control: &Control,
	) -> Result<workyard_core::Repository, Problem> {
		restoration::require_vacant_destination(&participant.destination)?;
		let repository = restoration::backing_repository(self, participant, control).await?;
		restoration::verify_registration(
			self,
			&repository,
			branch,
			&participant.destination,
			control,
		)
		.await?;
		Ok(repository.into())
	}

	async fn reconcile_recreation(
		&self,
		participant: &Participant,
		branch: &str,
		control: &Control,
	) -> Result<workyard_core::RecoveryObservation, Problem> {
		let mut participant = participant.clone();
		participant.phase = workyard_core::Phase::WorktreeCreated;
		let observed =
			workyard_core::RecoveryGit::reconcile(self, &participant, branch, control).await?;
		if observed.destination.is_none() || observed.operation_in_progress {
			return Err(blocked(
				"Successfully recreated worktree is missing or has unfinished Git work; inspect and repair it before recovery.",
			));
		}
		Ok(observed)
	}

	async fn reconcile_restored(
		&self,
		participant: &Participant,
		control: &Control,
	) -> Result<workyard_core::RecoveryObservation, Problem> {
		let facts = self.observe(participant, control).await?;
		if let Some(problem) = facts.problem {
			return Err(problem);
		}
		let destination = facts.path.ok_or_else(|| {
			blocked("Restored worktree is missing; preserve state and inspect Git.")
		})?;
		let source = if participant
			.source
			.try_exists()
			.map_err(|error| Problem::io("Inspect restored backing location", error))?
		{
			&participant.source
		} else {
			&destination
		};
		let repository = Repository::local(self, source, control).await?;
		if repository.identity != participant.backing_identity {
			return Err(blocked("Restored worktree backing identity changed."));
		}
		Ok(workyard_core::RecoveryObservation {
			repository: repository.into(),
			destination: Some(destination),
			operation_in_progress: facts.operation_in_progress || facts.conflicted,
		})
	}
}

impl Runner {
	/// Removal and copy activation serialize on the backing repository. Git resolves
	/// its worktree argument at execution, so verify its backlink while holding the gate.
	async fn verify_removal_location(
		&self,
		observation: &RemovalObservation,
		destination: &std::path::Path,
		control: &Control,
	) -> Result<std::path::PathBuf, Problem> {
		use crate::{
			relationships::pointer::{absolute, read_path},
			worktree_path::same_path,
		};
		let actual = destination
			.canonicalize()
			.map_err(|e| Problem::io("Resolve worktree before removal", e))?;
		if !same_path(&actual, destination)
			|| workyard_system::entry_identity(destination)? != observation.worktree_entry
		{
			return Err(blocked(
				"The physical worktree at the removal destination changed. Inspect it and retry.",
			));
		}
		let output = self
			.checked(
				destination,
				&args(&["rev-parse", "--path-format=absolute", "--git-dir"]),
				control,
			)
			.await?;
		let admin =
			crate::output::path_line(&output).map_err(|_| blocked("Invalid worktree path."))?;
		let backlink = absolute(&admin, &read_path(&admin.join("gitdir"))?);
		if !same_path(&backlink, &destination.join(".git"))
			|| workyard_system::directory_identity(&admin)? != observation.administration_identity
			|| workyard_system::directory_identity(&observation.repository.common_dir)?
				!= observation.repository.identity
		{
			return Err(blocked(
				"Git registration changed before removal. Inspect workyard status and retry.",
			));
		}
		Ok(admin)
	}

	async fn retained_history(
		&self,
		destination: &std::path::Path,
		branch: &str,
		control: &Control,
	) -> Result<RetainedHistory, Problem> {
		let upstream = self
			.checked(
				destination,
				&args(&[
					"for-each-ref",
					"--format=%(upstream)",
					&format!("refs/heads/{branch}"),
				]),
				control,
			)
			.await?;
		if upstream.iter().all(u8::is_ascii_whitespace) {
			return Ok(RetainedHistory::NoUpstream);
		}
		let ahead = self
			.checked(
				destination,
				&args(&["rev-list", "--count", "@{upstream}..HEAD"]),
				control,
			)
			.await?;
		if ahead == b"0\n" {
			Ok(RetainedHistory::UpToDate)
		} else {
			Ok(RetainedHistory::Unpushed)
		}
	}
}

fn blocked(message: impl Into<String>) -> Problem {
	Problem::new(ProblemCode::RepositoryChanged, message)
		.remedy("Inspect git status and git worktree list; preserve files and repair Git relationships before retrying. Force cannot bypass this check.")
}
