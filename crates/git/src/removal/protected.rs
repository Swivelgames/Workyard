//! An embedded clone or worktree has its own files and Git ownership.
use cap_primitives::fs::{self, FollowSymlinks};
use std::{fs::File, path::Path};
use workyard_core::{Problem, ProblemCode};

/// Inspect Git markers relative to the same directory handle used to enumerate children.
pub(super) fn opened_directory(dir: &File, path: &Path) -> Result<(), Problem> {
	let metadata = |name: &str| match fs::stat(dir, Path::new(name), FollowSymlinks::No) {
		Ok(metadata) => Ok(Some(metadata)),
		Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
		Err(error) => Err(Problem::io("Inspect nested Git marker", error)),
	};
	let nested = metadata(".git")?.is_some();
	let directory_or_link = |m: fs::Metadata| m.is_dir() || m.file_type().is_symlink();
	let bare = metadata("HEAD")?.is_some_and(|m| m.is_file() || m.file_type().is_symlink())
		&& metadata("objects")?.is_some_and(directory_or_link)
		&& metadata("refs")?.is_some_and(directory_or_link);
	if nested || bare {
		return Err(Problem::new(
			ProblemCode::PathOverlap,
			format!("Nested Git repository at {path:?}."),
		)
		.remedy("Move the nested repository outside the selected worktree before removal."));
	}
	Ok(())
}
