//! Verify that undo can recreate a participant at its recorded destination on its retained branch.
use super::blocked;
use crate::{Repository, Runner, process::args};
use std::path::Path;
use workyard_core::{Control, Participant, Problem};

pub(super) fn require_vacant_destination(destination: &Path) -> Result<(), Problem> {
	if destination
		.try_exists()
		.map_err(|e| Problem::io("Inspect restoration destination", e))?
	{
		return Err(blocked(
			"Restoration destination already exists; preserve it and choose a manual repair.",
		));
	}
	Ok(())
}

pub(super) async fn backing_repository(
	runner: &Runner,
	participant: &Participant,
	control: &Control,
) -> Result<Repository, Problem> {
	for path in [&participant.source, &participant.common_dir] {
		if !path
			.try_exists()
			.map_err(|e| Problem::io("Inspect backing repository", e))?
		{
			continue;
		}

		let repository = Repository::local(runner, path, control).await?;
		if repository.identity != participant.backing_identity {
			return Err(blocked("Backing repository identity changed."));
		}
		return Ok(repository);
	}
	Err(blocked("Backing repository is unreachable."))
}

pub(super) async fn verify_registration(
	runner: &Runner,
	repository: &Repository,
	branch: &str,
	destination: &Path,
	control: &Control,
) -> Result<(), Problem> {
	if crate::operations::operations_present(&repository.common_dir, &repository.common_dir)?
		|| repository
			.common_dir
			.join("refs/heads")
			.join(format!("{branch}.lock"))
			.try_exists()
			.map_err(|e| Problem::io("Inspect retained branch lock", e))?
	{
		return Err(blocked(
			"Backing repository has an unfinished Git operation.",
		));
	}

	runner
		.checked(
			&repository.source,
			&args(&["show-ref", "--verify", &format!("refs/heads/{branch}")]),
			control,
		)
		.await?;

	let listing = runner
		.checked(
			&repository.source,
			&args(&["worktree", "list", "--porcelain", "-z"]),
			control,
		)
		.await?;
	require_unused_registration(&listing, branch, destination)
}

fn require_unused_registration(
	listing: &[u8],
	branch: &str,
	destination: &Path,
) -> Result<(), Problem> {
	let expected = format!("branch refs/heads/{branch}");
	for field in listing.split(|byte| *byte == 0) {
		if field == expected.as_bytes() {
			return Err(blocked(
				"Retained branch is already used by another worktree.",
			));
		}
		let Some(path) = field.strip_prefix(b"worktree ") else {
			continue;
		};
		let registered =
			crate::output::path_field(path).map_err(|_| blocked("Invalid registration path."))?;
		if registered == destination {
			return Err(blocked(
				"Restoration destination still has a Git registration; repair it first.",
			));
		}
	}
	Ok(())
}
