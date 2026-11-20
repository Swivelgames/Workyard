mod contents;
mod index;

use crate::{Runner, process::args};
use sha2::{Digest, Sha256};
use std::{
	collections::BTreeSet,
	path::{Path, PathBuf},
};
use workyard_core::{Control, Problem};

/// Include file contents, not only porcelain status: editing an already-dirty
/// file while a prompt is visible requires a fresh decision too.
pub(super) async fn inspect(
	runner: &Runner,
	destination: &Path,
	worktree_entry: (u64, u64),
	control: &Control,
) -> Result<(Vec<PathBuf>, Vec<u8>), Problem> {
	let worktree = contents::Worktree::open(destination, worktree_entry)?;
	let status = runner
		.checked(
			destination,
			&args(&[
				"--no-optional-locks",
				"status",
				"--porcelain=v1",
				"-z",
				"--untracked-files=all",
				"--ignore-submodules=none",
				"--ignored=matching",
			]),
			control,
		)
		.await?;
	let mut paths = BTreeSet::new();
	let mut disclosed = BTreeSet::new();
	let mut fields = status
		.split(|byte| *byte == 0)
		.filter(|field| !field.is_empty());
	while let Some(field) = fields.next() {
		if field.len() < 4 {
			return Err(super::blocked("Invalid Git status output."));
		}
		let path = crate::output::path_field(&field[3..])
			.map_err(|_| super::blocked("Invalid dirty-file path."))?;
		// Porcelain terminates ignored directories with a slash. Keep the last
		// component inspectable as a symlink if it changes before traversal.
		let path: PathBuf = path.components().collect();
		if !matches!(&field[..2], b"??" | b"!!") {
			// A staged file may now be a directory with its own working descendants.
			disclosed.insert(path.clone());
		}
		paths.insert(path);
		if field[..2]
			.iter()
			.any(|status| matches!(status, b'R' | b'C'))
		{
			let original = fields
				.next()
				.ok_or_else(|| super::blocked("Missing rename source in Git status output."))?;
			if field[..2].contains(&b'R') {
				disclosed.insert(
					crate::output::path_field(original)
						.map_err(|_| super::blocked("Invalid rename source path."))?,
				);
			}
		}
	}

	let mut hash = Sha256::new();
	frame(&mut hash, &status);
	let index = runner
		.checked(
			destination,
			&args(&["ls-files", "-v", "--stage", "-z"]),
			control,
		)
		.await?;
	frame(&mut hash, &index);
	index::include_hidden_paths(&index, &worktree, &mut paths, control)?;
	for path in &paths {
		control.check()?;
		frame(&mut hash, path.as_os_str().as_encoded_bytes());
		let mut entry = Sha256::new();
		worktree.inspect(path, &mut disclosed, &mut entry, control)?;
		frame(&mut hash, &entry.finalize());
	}
	Ok((disclosed.into_iter().collect(), hash.finalize().to_vec()))
}

fn frame(hash: &mut Sha256, bytes: &[u8]) {
	hash.update((bytes.len() as u64).to_le_bytes());
	hash.update(bytes);
}
