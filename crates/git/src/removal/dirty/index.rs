//! Git status can suppress worktree bytes for entries marked assume-unchanged or skip-worktree.
use super::contents::Worktree;
use crate::removal::blocked;
use std::{collections::BTreeSet, path::PathBuf};
use workyard_core::{Control, Problem};

pub(super) fn include_hidden_paths(
	index: &[u8],
	worktree: &Worktree,
	paths: &mut BTreeSet<PathBuf>,
	control: &Control,
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
		if worktree.include_index_entry(&relative, gitlink, control)? {
			paths.insert(relative);
		}
	}
	Ok(())
}
