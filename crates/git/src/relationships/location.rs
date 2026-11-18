//! Locate a recorded administrative identity without requiring working .git pointers.
use super::changed;
use crate::output::path_line;
use std::path::{Path, PathBuf};
use workyard_core::{Participant, Problem, Repository};

pub(super) struct LinkedLocation {
	pub administration: PathBuf,
	pub destination: PathBuf,
	pub registration_matches: bool,
}

pub(super) fn locate(
	repository: &Repository,
	participant: &Participant,
) -> Result<LinkedLocation, Problem> {
	let identity = participant.worktree_identity.as_ref();
	let entries = std::fs::read_dir(repository.common_dir.join("worktrees"))
		.map_err(|error| {
			if error.kind() == std::io::ErrorKind::NotFound {
				return changed("The worktree administrative identity changed or no longer exists in this repository. Restore the missing worktree; recovery will not recreate authored work.");
			}
			Problem::io("Inspect linked-worktree administration", error)
		})?;
	for entry in entries {
		let entry = entry.map_err(|error| Problem::io("Read linked-worktree entry", error))?;
		let administration = entry.path();
		let observed_identity = workyard_system::directory_identity(&administration)?;
		if identity.is_some_and(|expected| expected != &observed_identity) {
			continue;
		}
		let registered = read_path(&administration.join("gitdir"))?;
		let registered = absolute(&administration, &registered);
		let registered = registered
			.parent()
			.ok_or_else(|| changed("Git worktree registration has no parent."))?;
		if identity.is_none() && registered != participant.destination {
			continue;
		}
		for destination in [registered, participant.destination.as_path()] {
			if !matches_pointer(destination, &administration, participant, std::fs::metadata)? {
				continue;
			}
			return Ok(LinkedLocation {
				administration,
				destination: destination
					.canonicalize()
					.map_err(|error| Problem::io("Resolve linked worktree location", error))?,
				registration_matches: same_path(destination, registered),
			});
		}
		if identity.is_some()
			&& let Some(destination) = discover_task_child(&administration, participant)?
		{
			return Ok(LinkedLocation {
				administration,
				destination,
				registration_matches: false,
			});
		}
		return Err(changed(
			"The participant worktree is missing. Restore it or supply its actual location through Git repair before retrying.",
		));
	}
	Err(changed(
		"The worktree administrative identity changed or no longer exists in this repository.",
	))
}

/// Search only immediate task children, using their linked administrative pointer.
fn discover_task_child(
	administration: &Path,
	participant: &Participant,
) -> Result<Option<PathBuf>, Problem> {
	let parent = participant
		.destination
		.parent()
		.ok_or_else(|| changed("Participant has no parent directory."))?;
	if !parent
		.try_exists()
		.map_err(|error| Problem::io("Inspect task directory", error))?
	{
		return Ok(None);
	}
	let mut matches = Vec::new();
	for entry in
		std::fs::read_dir(parent).map_err(|error| Problem::io("Read task directory", error))?
	{
		let entry = entry.map_err(|error| Problem::io("Read task child", error))?;
		if !entry
			.file_type()
			.map_err(|error| Problem::io("Inspect task child", error))?
			.is_dir()
		{
			continue;
		}
		if matches_pointer(
			&entry.path(),
			administration,
			participant,
			std::fs::symlink_metadata,
		)? {
			matches.push(entry.path());
		}
	}
	if matches.len() > 1 {
		return Err(changed(
			"Several task children refer to this worktree identity. Preserve all files and explicitly repair Git at the intended location.",
		));
	}
	Ok(matches.pop())
}

fn matches_pointer(
	destination: &Path,
	administration: &Path,
	participant: &Participant,
	metadata: impl FnOnce(PathBuf) -> std::io::Result<std::fs::Metadata>,
) -> Result<bool, Problem> {
	let pointer = destination.join(".git");
	let metadata = match metadata(pointer) {
		Ok(metadata) => metadata,
		Err(error)
			if matches!(
				error.kind(),
				std::io::ErrorKind::NotFound | std::io::ErrorKind::NotADirectory
			) =>
		{
			return Ok(false);
		}
		Err(error) => return Err(Problem::io("Inspect linked-worktree pointer", error)),
	};
	if !metadata.is_file() {
		return Ok(false);
	}

	match verify_pointer(destination, administration, participant) {
		Ok(()) => Ok(true),
		Err(problem) if problem.code == workyard_core::ProblemCode::RepositoryChanged => Ok(false),
		Err(problem) => Err(problem),
	}
}

fn verify_pointer(
	destination: &Path,
	administration: &Path,
	participant: &Participant,
) -> Result<(), Problem> {
	let bytes = std::fs::read(destination.join(".git"))
		.map_err(|error| Problem::io("Read linked-worktree pointer", error))?;
	let pointer = bytes
		.strip_prefix(b"gitdir: ")
		.ok_or_else(|| changed("The participant is not a Git linked worktree."))?;
	let path = path_line(pointer).map_err(|_| changed("Invalid native Git pointer."))?;
	let path = absolute(destination, &path);
	let former = participant
		.common_dir
		.join("worktrees")
		.join(administration.file_name().expect("administrative entry"));
	if !same_path(&path, administration) && !same_path(&path, &former) {
		return Err(changed(
			"Participant Git pointer refers to different administrative storage.",
		));
	}
	Ok(())
}

pub(super) fn read_path(path: &Path) -> Result<PathBuf, Problem> {
	let bytes = std::fs::read(path)
		.map_err(|error| Problem::io("Read Git worktree registration", error))?;
	path_line(&bytes).map_err(|_| changed("Invalid native Git registration."))
}

pub(super) fn absolute(base: &Path, path: &Path) -> PathBuf {
	let joined = if path.is_absolute() {
		path.to_path_buf()
	} else {
		base.join(path)
	};
	let mut result = PathBuf::new();
	for component in joined.components() {
		match component {
			std::path::Component::CurDir => {}
			std::path::Component::ParentDir => {
				result.pop();
			}
			component => result.push(component.as_os_str()),
		}
	}
	result
}

/// Git for Windows can omit the verbatim prefix on an otherwise identical path.
pub(super) fn same_path(left: &Path, right: &Path) -> bool {
	if left == right {
		return true;
	}
	#[cfg(windows)]
	{
		use std::path::{Component, Prefix};

		let mut left = left.components();
		let mut right = right.components();
		let (Some(Component::Prefix(a)), Some(Component::Prefix(b))) = (left.next(), right.next())
		else {
			return false;
		};
		let prefixes_match = match (a.kind(), b.kind()) {
			(
				Prefix::Disk(a) | Prefix::VerbatimDisk(a),
				Prefix::Disk(b) | Prefix::VerbatimDisk(b),
			) => a.eq_ignore_ascii_case(&b),
			(Prefix::UNC(a, b), Prefix::VerbatimUNC(c, d))
			| (Prefix::VerbatimUNC(a, b), Prefix::UNC(c, d)) => a == c && b == d,
			_ => false,
		};
		return prefixes_match && left.eq(right);
	}
	#[cfg(not(windows))]
	false
}
