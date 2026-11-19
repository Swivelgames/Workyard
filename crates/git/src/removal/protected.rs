//! An embedded clone or worktree has its own files and Git ownership.
use std::{
	collections::BTreeSet,
	path::{Path, PathBuf},
};
use workyard_core::{Problem, ProblemCode};

pub(super) fn directory(path: &Path) -> Result<(), Problem> {
	let marker = path.join(".git");
	let nested = match std::fs::symlink_metadata(&marker) {
		Ok(_) => true,
		Err(error)
			if matches!(
				error.kind(),
				std::io::ErrorKind::NotFound | std::io::ErrorKind::NotADirectory
			) =>
		{
			false
		}
		Err(error) => return Err(Problem::io("Inspect nested Git marker", error)),
	};
	let bare =
		path.join("HEAD").is_file() && path.join("objects").is_dir() && path.join("refs").is_dir();
	if nested || bare {
		return Err(Problem::new(
			ProblemCode::PathOverlap,
			format!("Nested Git repository at {path:?}."),
		)
		.remedy("Move the nested repository outside the selected worktree before removal."));
	}
	Ok(())
}

/// Porcelain may enumerate individual files inside an untracked bare repository.
pub(super) fn parents(destination: &Path, paths: &BTreeSet<PathBuf>) -> Result<(), Problem> {
	let mut seen = BTreeSet::new();
	for path in paths {
		let mut parent = path.parent();
		while let Some(relative) = parent.filter(|p| !p.as_os_str().is_empty()) {
			if !seen.insert(relative) {
				break;
			}
			directory(&destination.join(relative))?;
			parent = relative.parent();
		}
	}
	Ok(())
}
