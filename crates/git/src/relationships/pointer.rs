//! Read both directions of Git's native worktree links.
use super::changed;
use crate::output::path_line;
use std::path::{Path, PathBuf};
use workyard_core::{PlacementHint, Problem};

pub(super) fn matches_pointer(
	destination: &Path,
	administration: &Path,
	previous: Option<&PlacementHint>,
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

	match verify_pointer(destination, administration, previous) {
		Ok(()) => Ok(true),
		Err(problem) if problem.code == workyard_core::ProblemCode::RepositoryChanged => Ok(false),
		Err(problem) => Err(problem),
	}
}

fn verify_pointer(
	destination: &Path,
	administration: &Path,
	previous: Option<&PlacementHint>,
) -> Result<(), Problem> {
	let bytes = std::fs::read(destination.join(".git"))
		.map_err(|error| Problem::io("Read linked-worktree pointer", error))?;
	let pointer = bytes
		.strip_prefix(b"gitdir: ")
		.ok_or_else(|| changed("The participant is not a Git linked worktree."))?;
	let pointer = path_line(pointer).map_err(|_| changed("Invalid native Git pointer."))?;
	let current = absolute(destination, &pointer);
	if same_path(&current, administration) {
		return Ok(());
	}
	if current
		.try_exists()
		.map_err(|e| Problem::io("Inspect Git pointer", e))?
	{
		return Err(changed(
			"Participant Git pointer refers to different administrative storage.",
		));
	}

	// Relative links copied to another depth keep their original bytes.
	// Resolve them from the last verified tree only if the current target is gone.
	let hint = previous.ok_or_else(|| changed("The previous worktree location is unknown."))?;
	let previous = absolute(&hint.destination, &pointer);
	if same_path(&previous, administration) {
		return Ok(());
	}
	let former_admin = hint
		.common_dir
		.join("worktrees")
		.join(administration.file_name().expect("administrative entry"));
	if same_path(&previous, &former_admin)
		&& !previous
			.try_exists()
			.map_err(|e| Problem::io("Inspect previous Git pointer", e))?
	{
		return Ok(());
	}
	Err(changed(
		"Participant Git pointer refers to different administrative storage.",
	))
}

pub(crate) fn read_path(path: &Path) -> Result<PathBuf, Problem> {
	let bytes = std::fs::read(path)
		.map_err(|error| Problem::io("Read Git worktree registration", error))?;
	path_line(&bytes).map_err(|_| changed("Invalid native Git registration."))
}

pub(crate) fn absolute(base: &Path, path: &Path) -> PathBuf {
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
pub(crate) fn same_path(left: &Path, right: &Path) -> bool {
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
		prefixes_match && left.eq(right)
	}
	#[cfg(not(windows))]
	false
}
