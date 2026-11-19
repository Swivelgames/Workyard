//! Native Git also repairs links at the other registered paths in this repository.
use super::pointer::{absolute, read_path, same_path};
use crate::{Runner, operations::writer_lock_in, output::path_line, process::args};
use std::{collections::HashSet, fs, path::Path};
use workyard_core::{Control, Participant, Problem, ProblemCode, Repository};

impl Runner {
	/// Caller holds the repository relationship lock and has verified the selected tree.
	pub(super) async fn repair_registered_tree(
		&self,
		repo: &Repository,
		participant: &Participant,
		selected_admin: &Path,
		destination: &Path,
		unavailable_worktrees: Option<&HashSet<String>>,
		control: &Control,
	) -> Result<(), Problem> {
		let previous_common = participant
			.placement
			.as_ref()
			.map_or(participant.common_dir.as_path(), |hint| {
				hint.common_dir.as_path()
			});
		let entries = fs::read_dir(repo.common_dir.join("worktrees"))
			.map_err(|e| Problem::io("Inspect worktrees before Git repair", e))?;

		for entry in entries {
			control.check()?;
			let admin = entry
				.map_err(|e| Problem::io("Inspect worktree entry", e))?
				.path();
			if !same_path(&admin, selected_admin) && exists(&admin.join("gitdir"))? {
				verify_peer(&admin, previous_common, unavailable_worktrees)?;
			}
		}

		control.check()?;
		let mut arguments = args(&["worktree", "repair", "--"]);
		arguments.push(destination.as_os_str().into());
		self.effect(&repo.source, &arguments, control).await?;
		Ok(())
	}
}

fn verify_peer(
	admin: &Path,
	previous_common: &Path,
	unavailable_worktrees: Option<&HashSet<String>>,
) -> Result<(), Problem> {
	let pointer = absolute(admin, &read_path(&admin.join("gitdir"))?);
	let Some(tree) = pointer
		.parent()
		.filter(|_| pointer.file_name().is_some_and(|name| name == ".git"))
	else {
		return Err(super::changed(
			"Another worktree has an invalid Git registration.",
		));
	};
	// Git skips a missing worktree but can create or replace .git in an existing directory.
	if !exists(tree)? {
		return Ok(());
	}
	let physical = tree
		.canonicalize()
		.map_err(|error| Problem::io("Resolve registered peer worktree", error))?;
	if !same_path(tree, &physical) {
		return Err(uncertain_peer(tree));
	}

	if writer_lock_in(admin)? {
		return Err(Problem::new(
			ProblemCode::ResourceBusy,
			format!("Another registered worktree at {tree:?} has a Git writer lock."),
		)
		.remedy("Retry workspace recovery after the Git operation finishes."));
	}

	if let Some(unavailable) = unavailable_worktrees
		&& !unavailable.is_empty()
		&& unavailable.contains(&workyard_system::directory_identity(admin)?)
	{
		return Err(Problem::new(
			ProblemCode::ResourceBusy,
			format!("Git repair would also change an unresolved worktree at {tree:?}."),
		)
		.remedy("Resolve the reported worktree condition and retry recovery."));
	}

	let metadata = fs::symlink_metadata(&pointer);
	if !metadata.as_ref().is_ok_and(|entry| entry.is_file()) {
		return Err(uncertain_peer(tree));
	}
	let bytes = fs::read(&pointer).map_err(|e| Problem::io("Read peer worktree pointer", e))?;
	let path = bytes
		.strip_prefix(b"gitdir: ")
		.and_then(|value| path_line(value).ok())
		.ok_or_else(|| uncertain_peer(tree))?;
	let target = absolute(tree, &path);
	if same_path(&target, admin) {
		return Ok(());
	}

	// Moving a clone can leave its other worktrees pointing into the old clone.
	// A proven stale link can be reconnected to the same administrative entry.
	let previous = previous_common
		.join("worktrees")
		.join(admin.file_name().expect("worktree entry"));
	if !same_path(&target, &previous) || exists(&previous)? || exists(&admin.join("locked"))? {
		return Err(uncertain_peer(tree));
	}

	if unavailable_worktrees.is_none() {
		return Err(Problem::new(
			ProblemCode::RepositoryChanged,
			format!("Git would also reconnect the worktree at {tree:?}."),
		)
		.remedy("Run workyard recover to check the other worktrees in this workspace."));
	}
	Ok(())
}

fn exists(path: &Path) -> Result<bool, Problem> {
	path.try_exists()
		.map_err(|e| Problem::io(format!("Inspect {path:?} before Git repair"), e))
}

fn uncertain_peer(tree: &Path) -> Problem {
	Problem::new(
		ProblemCode::RepositoryChanged,
		format!("Git repair would also write an unverified worktree link at {tree:?}."),
	)
	.remedy("Check that other worktree’s registration before retrying recovery.")
}
