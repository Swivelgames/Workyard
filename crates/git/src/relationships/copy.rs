//! Inspect both ends of a copied worktree before rebinding its shared registration.
use super::{
	changed,
	location::{absolute, read_path, same_path},
	relationship_lock,
};
use crate::{Runner, operations::locks_present, output::path_line, process::args};
use std::{fs, path::Path};
use workyard_core::{
	Control, Git, Problem, ProblemCode, RegisteredCopy, RelocationParticipant, Repository,
};

fn endpoint(entry: &RelocationParticipant, undo: bool) -> &Path {
	if undo {
		&entry.original.destination
	} else {
		&entry.destination
	}
}

fn repository(entry: &RelocationParticipant) -> Repository {
	let participant = &entry.original;
	Repository {
		source: participant.source.clone(),
		common_dir: participant.common_dir.clone(),
		remote: participant.remote.clone(),
		identity: participant.backing_identity.clone(),
	}
}

pub(super) async fn inspect(
	runner: &Runner,
	entry: &RelocationParticipant,
	undo: bool,
	control: &Control,
) -> Result<bool, Problem> {
	let target = endpoint(entry, undo);
	let repo = repository(entry);
	let result = runner
		.revalidate(&repo, control)
		.await
		.and_then(|()| registration_matches(entry, target, &repo));
	result.map_err(|mut problem| {
		problem.message = format!("{} at {target:?}: {}", entry.original.name, problem.message);
		problem
	})
}

fn registration(
	entry: &RelocationParticipant,
	repo: &Repository,
) -> Result<RegisteredCopy, Problem> {
	let admins = fs::read_dir(repo.common_dir.join("worktrees"))
		.map_err(|e| Problem::io("Read worktree registrations", e))?;
	for admin in admins {
		let admin = admin
			.map_err(|e| Problem::io("Read worktree registration", e))?
			.path();
		if Some(workyard_system::directory_identity(&admin)?) != entry.original.worktree_identity {
			continue;
		}
		if locks_present(&admin, &repo.common_dir)? {
			return Err(Problem::new(
				ProblemCode::ResourceBusy,
				"Git has an active writer lock.",
			));
		}
		let gitfile = absolute(&admin, &read_path(&admin.join("gitdir"))?);
		if gitfile.file_name().is_none_or(|name| name != ".git") {
			return Err(changed("Invalid worktree registration pointer."));
		}
		let registered = gitfile
			.parent()
			.ok_or_else(|| changed("Invalid worktree registration."))?;
		return Ok(RegisteredCopy {
			bound_at: registered.to_owned(),
			path: registered.to_owned(),
			administration: admin,
		});
	}
	Err(changed("Recorded Git administrative identity is missing."))
}

pub(super) async fn displaced(
	runner: &Runner,
	entry: &RelocationParticipant,
	undo: bool,
	previous: Option<&Path>,
	control: &Control,
) -> Result<Option<RegisteredCopy>, Problem> {
	let repo = repository(entry);
	runner.revalidate(&repo, control).await?;
	let mut registered = registration(entry, &repo)?;
	let target = endpoint(entry, undo);
	let alternate = endpoint(entry, !undo);
	if same_path(&registered.path, target) {
		return Ok(None);
	}
	if same_path(&registered.path, alternate) {
		let Some(previous) = previous else {
			return Ok(None);
		};
		if !pointer_matches(alternate, entry, &registered.administration)? {
			return Err(changed(
				"Registered worktree has an unresolved Git pointer.",
			));
		}
		registered.path = if previous
			.try_exists()
			.map_err(|e| Problem::io("Inspect previous participant location", e))?
		{
			previous.to_owned()
		} else {
			alternate.to_owned()
		};
	}
	if !pointer_matches(&registered.path, entry, &registered.administration)? {
		return Err(changed(
			"Manually registered worktree has an unresolved Git pointer.",
		));
	}
	let resolved = registered
		.path
		.canonicalize()
		.map_err(|e| Problem::io("Resolve registered worktree", e))?;
	if !same_path(&registered.path, &resolved) {
		return Err(changed(
			"Registered worktree path traverses a redirected directory.",
		));
	}
	registered.path = resolved;
	let target = endpoint(entry, undo);
	let alternate = endpoint(entry, !undo);
	for path in [target, alternate] {
		match fs::symlink_metadata(path) {
			Ok(metadata) if metadata.is_dir() => {
				// An unresolvable relative pointer is eligible only when it still
				// identifies the original administration from its former root.
				pointer_matches(path, entry, &registered.administration)?;
			}
			Ok(_) => return Err(changed("Recovery candidate is not a worktree directory.")),
			Err(e) if e.kind() == std::io::ErrorKind::NotFound => (),
			Err(e) => return Err(Problem::io("Inspect recovery candidate", e)),
		}
	}
	Ok(Some(registered))
}

fn registration_matches(
	entry: &RelocationParticipant,
	target: &Path,
	repo: &Repository,
) -> Result<bool, Problem> {
	let registered = registration(entry, repo)?;
	if !same_path(&registered.path, &entry.original.destination)
		&& !same_path(&registered.path, &entry.destination)
	{
		return Err(changed(&format!(
			"Git registered this worktree at {:?} outside the copy endpoints.",
			registered.path
		)));
	}
	if !same_path(&registered.path, target)
		&& !pointer_matches(&registered.path, entry, &registered.administration)?
	{
		return Err(changed(
			"Registered worktree has an unresolved Git pointer.",
		));
	}
	let matches = pointer_matches(target, entry, &registered.administration)?;
	Ok(same_path(&registered.path, target) && matches)
}

pub(super) async fn repair(
	runner: &Runner,
	entry: &RelocationParticipant,
	undo: bool,
	expected: Option<&RegisteredCopy>,
	control: &Control,
) -> Result<(), Problem> {
	let repo = repository(entry);
	let _gate = relationship_lock(&repo, control).await?;
	let target = endpoint(entry, undo);
	if let Some(expected) = expected {
		runner.revalidate(&repo, control).await?;
		let current = registration(entry, &repo)?;
		if !same_path(&current.path, &expected.bound_at)
			|| !same_path(&current.administration, &expected.administration)
			|| !pointer_matches(&expected.path, entry, &expected.administration)?
		{
			return Err(changed("Git registration changed during recovery."));
		}
		pointer_matches(target, entry, &expected.administration)?;
	} else if inspect(runner, entry, undo, control).await? {
		return Ok(());
	}
	let mut arguments = args(&["worktree", "repair", "--"]);
	arguments.push(target.as_os_str().into());
	runner.effect(&repo.source, &arguments, control).await?;
	if !inspect(runner, entry, undo, &Control::new(|| false)).await? {
		return Err(changed(&format!(
			"Git repair did not bind {} at {target:?}.",
			entry.original.name
		)));
	}
	Ok(())
}

fn pointer_matches(
	target: &Path,
	entry: &RelocationParticipant,
	admin: &Path,
) -> Result<bool, Problem> {
	let pointer = target.join(".git");
	if !fs::symlink_metadata(&pointer)
		.map_err(|e| Problem::io("Inspect linked worktree pointer", e))?
		.is_file()
	{
		return Err(changed("Expected a regular linked worktree pointer."));
	}
	let bytes = fs::read(pointer).map_err(|e| Problem::io("Read linked worktree pointer", e))?;
	let raw = bytes
		.strip_prefix(b"gitdir: ")
		.ok_or_else(|| changed("Expected a linked worktree pointer."))?;
	let pointer = path_line(raw).map_err(|_| changed("Invalid linked worktree pointer."))?;
	let actual = absolute(target, &pointer);
	if !same_path(&actual, admin) {
		// A copied relative pointer can still be expressed from the original directory.
		if actual
			.try_exists()
			.map_err(|e| Problem::io("Inspect copied Git pointer", e))?
			|| !same_path(&absolute(&entry.original.destination, &pointer), admin)
		{
			return Err(changed(
				"Worktree pointer refers to different administrative storage.",
			));
		}
	}
	Ok(same_path(&actual, admin))
}
