use std::path::Path;
use workyard_core::Problem;

pub(super) fn operations_present(git_dir: &Path, common: &Path) -> Result<bool, Problem> {
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
