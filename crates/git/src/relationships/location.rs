//! Locate a recorded administrative identity without requiring working .git pointers.
use super::changed;
use crate::output::path_line;
use std::path::{Path, PathBuf};
use workyard_core::{Participant, Problem, Repository};

pub(super) struct LinkedLocation {
	pub administration: PathBuf,
	pub destination: PathBuf,
}

pub(super) fn locate(
	repository: &Repository,
	participant: &Participant,
) -> Result<LinkedLocation, Problem> {
	let identity = participant.worktree_identity.as_ref();
	let entries = std::fs::read_dir(repository.common_dir.join("worktrees"))
		.map_err(|error| {
			if error.kind() == std::io::ErrorKind::NotFound {
				return changed("The recorded worktree administration no longer exists in this repository. Restore the missing worktree; recovery will not recreate authored work.");
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
			if !destination
				.try_exists()
				.map_err(|error| Problem::io("Inspect linked-worktree location", error))?
			{
				continue;
			}
			verify_pointer(destination, &administration, participant)?;
			return Ok(LinkedLocation {
				administration,
				destination: destination.to_path_buf(),
			});
		}
		return Err(changed(
			"The participant worktree is missing. Restore it or supply its actual location through Git repair before retrying.",
		));
	}
	Err(changed(
		"The recorded worktree administration no longer exists in this repository.",
	))
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
	if path != administration && path != former {
		return Err(changed(
			"Participant Git pointer refers to different administrative storage.",
		));
	}
	Ok(())
}

fn read_path(path: &Path) -> Result<PathBuf, Problem> {
	let bytes = std::fs::read(path)
		.map_err(|error| Problem::io("Read Git worktree registration", error))?;
	path_line(&bytes).map_err(|_| changed("Invalid native Git registration."))
}

fn absolute(base: &Path, path: &Path) -> PathBuf {
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
