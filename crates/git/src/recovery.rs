mod registrations;

use crate::operations::operations_present;
use crate::{Repository, Runner, process::args};
use registrations::registrations;
use std::path::PathBuf;
use workyard_core::{Control, Participant, Problem, ProblemCode, RecoveryGit, RecoveryObservation};

struct Reconciliation<'a> {
	runner: &'a Runner,
	participant: &'a Participant,
	task: &'a str,
	control: &'a Control,
}

impl RecoveryGit for Runner {
	async fn recovery_relationship(
		&self,
		participant: &Participant,
		control: &Control,
	) -> Result<Option<workyard_core::Relationship>, Problem> {
		if participant.worktree_identity.is_none() {
			return Ok(None);
		}
		let reconciliation = Reconciliation {
			runner: self,
			participant,
			task: "",
			control,
		};
		let repository = reconciliation.locate_repository().await?;
		workyard_core::AmendGit::relationship(self, &repository.into(), participant, control)
			.await
			.map(Some)
	}

	async fn repair_relationship(
		&self,
		repository: &workyard_core::Repository,
		participant: &Participant,
		control: &Control,
	) -> Result<(), Problem> {
		workyard_core::AmendGit::repair(self, repository, participant, control).await
	}

	async fn reconcile(
		&self,
		participant: &Participant,
		task: &str,
		control: &Control,
	) -> Result<RecoveryObservation, Problem> {
		Reconciliation {
			runner: self,
			participant,
			task,
			control,
		}
		.run()
		.await
	}
}

impl Reconciliation<'_> {
	async fn run(&self) -> Result<RecoveryObservation, Problem> {
		let repository = self.locate_repository().await?;
		let destination = self.registered_destination(&repository).await?;
		let operation_in_progress = match &destination {
			Some(path) => self.inspect_registered(&repository, path).await?,
			None => {
				if self
					.participant
					.destination
					.try_exists()
					.map_err(|error| Problem::io("Inspect participant destination", error))?
				{
					return Err(changed(
						"Participant destination exists without its registered task worktree. Inspect git worktree list and repair registration; do not replace its files.",
					));
				}
				operations_present(&repository.common_dir, &repository.common_dir)?
			}
		};
		Ok(RecoveryObservation {
			repository: repository.into(),
			destination,
			operation_in_progress,
		})
	}

	async fn locate_repository(&self) -> Result<Repository, Problem> {
		// A repaired linked worktree can locate moved backing storage.
		for path in [
			&self.participant.source,
			&self.participant.destination,
			&self.participant.common_dir,
		] {
			if !path
				.try_exists()
				.map_err(|error| Problem::io("Inspect cached Git location", error))?
			{
				continue;
			}

			let repository = Repository::inspect(
				self.runner,
				path,
				self.participant.remote.as_deref(),
				self.control,
			)
			.await?;
			if repository.identity != self.participant.backing_identity {
				return Err(changed(
					"Cached Git location points to different backing storage.",
				));
			}

			return Ok(repository);
		}

		Err(changed(
			"Backing Git storage is unreachable. Repair moved Git relationships with git worktree repair from the actual main clone, then retry.",
		))
	}

	async fn registered_destination(
		&self,
		repository: &Repository,
	) -> Result<Option<PathBuf>, Problem> {
		let output = self
			.runner
			.checked(
				&repository.source,
				&args(&["worktree", "list", "--porcelain", "-z"]),
				self.control,
			)
			.await?;
		let branch = format!("refs/heads/{}", self.task);
		let matching: Vec<_> = registrations(&output)?
			.into_iter()
			.filter(|(_, candidate)| candidate == &branch)
			.collect();
		if matching.len() > 1 {
			return Err(changed(
				"Several worktrees use the task branch; ownership is ambiguous.",
			));
		}

		Ok(matching.into_iter().next().map(|(path, _)| path))
	}

	async fn inspect_registered(
		&self,
		repository: &Repository,
		path: &std::path::Path,
	) -> Result<bool, Problem> {
		let participant = self.participant;
		// Branch spelling alone cannot establish ownership of an unstarted tree.
		if matches!(
			participant.phase,
			workyard_core::Phase::NotStarted | workyard_core::Phase::Fetching
		) || (participant.phase == workyard_core::Phase::Creating
			&& participant.worktree_identity.is_none())
		{
			return Err(changed(
				"A worktree already uses a not-started participant branch; recovery cannot adopt it.",
			));
		}

		let current = Repository::inspect(
			self.runner,
			path,
			participant.remote.as_deref(),
			self.control,
		)
		.await?;
		if current.identity != participant.backing_identity {
			return Err(changed("Registered worktree backing identity differs."));
		}

		let output = self
			.runner
			.checked(
				path,
				&args(&["rev-parse", "--path-format=absolute", "--git-dir"]),
				self.control,
			)
			.await?;
		let git_dir =
			crate::output::path_line(&output).map_err(|_| changed("Invalid Git path output."))?;
		if let Some(expected) = &participant.worktree_identity {
			if &workyard_system::directory_identity(&git_dir)? != expected {
				return Err(changed(
					"Worktree administrative identity changed; recovery cannot adopt its replacement.",
				));
			}
		} else if path != participant.destination {
			return Err(changed(
				"Legacy worktree has no administrative identity evidence to verify an external move.",
			));
		}

		let conflicts = self
			.runner
			.checked(path, &args(&["ls-files", "--unmerged", "-z"]), self.control)
			.await?;
		let branch_lock = repository
			.common_dir
			.join("refs/heads")
			.join(format!("{}.lock", self.task));
		Ok(operations_present(&git_dir, &repository.common_dir)?
			|| !conflicts.is_empty()
			|| branch_lock
				.try_exists()
				.map_err(|error| Problem::io("Inspect task branch lock", error))?)
	}
}

fn changed(message: &str) -> Problem {
	Problem::new(ProblemCode::RepositoryChanged, message).remedy("Inspect git worktree list --porcelain and git status at the recorded paths. Repair Git relationships manually; preserve worktrees and native Git locks.")
}
