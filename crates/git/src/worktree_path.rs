//! Compare Git's worktree paths with recorded locations.
use std::path::{Path, PathBuf};

/// Keep the recorded spelling when Git omits an optional Windows verbatim prefix.
pub(crate) fn registered_path(path: PathBuf, recorded: &Path) -> PathBuf {
	if same_path(&path, recorded) {
		recorded.to_owned()
	} else {
		path
	}
}

pub(crate) fn same_path(left: &Path, right: &Path) -> bool {
	if left == right {
		return true;
	}
	#[cfg(windows)]
	{
		same_windows_location(left, right)
	}
	#[cfg(not(windows))]
	false
}

#[cfg(windows)]
fn same_windows_location(left: &Path, right: &Path) -> bool {
	use std::path::{Component, Prefix};

	let mut left_parts = left.components();
	let mut right_parts = right.components();
	let (Some(Component::Prefix(a)), Some(Component::Prefix(b))) =
		(left_parts.next(), right_parts.next())
	else {
		return false;
	};
	let same_volume = match (a.kind(), b.kind()) {
		(Prefix::Disk(a), Prefix::VerbatimDisk(b)) | (Prefix::VerbatimDisk(a), Prefix::Disk(b)) => {
			a.eq_ignore_ascii_case(&b)
		}
		(Prefix::UNC(a, b), Prefix::VerbatimUNC(c, d))
		| (Prefix::VerbatimUNC(a, b), Prefix::UNC(c, d)) => a == c && b == d,
		_ => false,
	};
	if !same_volume || !left_parts.eq(right_parts) {
		return false;
	}

	// The ordinary namespace can trim trailing dots or resolve reserved names.
	// Only the prefix may differ when adopting the recorded spelling.
	let ordinary = if a.kind().is_verbatim() { right } else { left };
	std::path::absolute(ordinary).is_ok_and(|resolved| resolved == ordinary)
}

#[cfg(all(test, windows))]
mod tests {
	use super::*;

	#[test]
	fn equivalent_git_paths_keep_the_recorded_spelling() {
		let long = r"participant\".repeat(25);
		for (git, recorded) in [
			(r"C:/tasks/api".to_owned(), r"\\?\C:\tasks\api".to_owned()),
			(format!(r"C:\{long}api"), format!(r"\\?\C:\{long}api")),
			(
				r"//server/share/api".to_owned(),
				r"\\?\UNC\server\share\api".to_owned(),
			),
		] {
			let chosen = registered_path(PathBuf::from(git), Path::new(&recorded));
			assert_eq!(chosen.as_os_str(), std::ffi::OsStr::new(&recorded));
		}
	}

	#[test]
	fn different_worktree_locations_keep_the_git_path() {
		for (git, recorded) in [
			(r"C:\tasks\web", r"\\?\C:\tasks\api"),
			(r"D:\tasks\api", r"\\?\C:\tasks\api"),
			(r"\\server\other\api", r"\\?\UNC\server\share\api"),
			(r"C:\tasks\api.", r"\\?\C:\tasks\api."),
			(r"\\.\NUL", r"\\?\C:\tasks\NUL"),
		] {
			let chosen = registered_path(PathBuf::from(git), Path::new(recorded));
			assert_eq!(chosen.as_os_str(), std::ffi::OsStr::new(git));
		}
	}
}
