//! Git status can suppress worktree bytes for entries marked assume-unchanged or skip-worktree.
use super::{blocked, protected};
use std::{
	collections::BTreeSet,
	path::{Path, PathBuf},
};
use workyard_core::Problem;

pub(super) fn include_hidden_paths(
	index: &[u8],
	destination: &Path,
	paths: &mut BTreeSet<PathBuf>,
) -> Result<(), Problem> {
	for entry in index
		.split(|byte| *byte == 0)
		.filter(|entry| !entry.is_empty())
	{
		// ls-files -v --stage -z emits "<tag> <mode> <object> <stage> TAB <path> NUL".
		let separator = entry
			.iter()
			.position(|b| *b == b'\t')
			.ok_or_else(|| blocked("Invalid Git index entry."))?;
		let metadata = &entry[..separator];
		let flagged = metadata
			.first()
			.is_some_and(|tag| tag.is_ascii_lowercase() || *tag == b'S');
		let gitlink = metadata.get(2..8) == Some(b"160000");
		if !flagged && !gitlink {
			continue;
		}
		let relative = crate::output::path_field(&entry[separator + 1..])
			.map_err(|_| blocked("Invalid Git index path."))?;
		let absolute = destination.join(&relative);
		match std::fs::symlink_metadata(&absolute) {
			Ok(metadata) => {
				if gitlink && metadata.is_dir() {
					protected::directory(&absolute)?;
				}
				if flagged {
					paths.insert(relative);
				}
			}
			Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
			Err(error) => return Err(Problem::io("Inspect Git index path", error)),
		}
	}
	Ok(())
}
