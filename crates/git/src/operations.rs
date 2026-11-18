use std::path::Path;
use workyard_core::Problem;

pub(crate) fn operations_present(git_dir: &Path, common: &Path) -> Result<bool, Problem> {
	for directory in [git_dir, common] {
		for name in [
			"index.lock",
			"HEAD.lock",
			"packed-refs.lock",
			"shallow.lock",
			"config.lock",
			"FETCH_HEAD.lock",
			"ORIG_HEAD.lock",
			"MERGE_HEAD",
			"CHERRY_PICK_HEAD",
			"REVERT_HEAD",
			"rebase-merge",
			"rebase-apply",
			"sequencer",
		] {
			if directory
				.join(name)
				.try_exists()
				.map_err(|e| Problem::io("Inspect Git operation state", e))?
			{
				return Ok(true);
			}
		}
	}
	Ok(false)
}

/// Native writer locks exclude static merge/rebase state, which relocation preserves.
pub(crate) fn locks_present(git_dir: &Path, common: &Path) -> Result<bool, Problem> {
	for directory in [git_dir, common] {
		for name in [
			"index.lock",
			"HEAD.lock",
			"packed-refs.lock",
			"shallow.lock",
			"config.lock",
			"FETCH_HEAD.lock",
			"ORIG_HEAD.lock",
		] {
			if directory
				.join(name)
				.try_exists()
				.map_err(|error| Problem::io("Inspect Git writer lock", error))?
			{
				return Ok(true);
			}
		}
		if ref_locks_present(&directory.join("refs"))? {
			return Ok(true);
		}
	}
	Ok(false)
}

/// Ref names are nested paths; inspect actual storage without following symlinks.
fn ref_locks_present(directory: &Path) -> Result<bool, Problem> {
	let entries = match std::fs::read_dir(directory) {
		Ok(entries) => entries,
		Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(false),
		Err(error) => return Err(Problem::io("Inspect Git ref locks", error)),
	};
	for entry in entries {
		let entry = entry.map_err(|error| Problem::io("Read Git ref entry", error))?;
		let kind = entry
			.file_type()
			.map_err(|error| Problem::io("Inspect Git ref entry", error))?;
		if entry
			.path()
			.extension()
			.is_some_and(|extension| extension == "lock")
		{
			return Ok(true);
		}
		if kind.is_dir() && ref_locks_present(&entry.path())? {
			return Ok(true);
		}
	}
	Ok(false)
}
