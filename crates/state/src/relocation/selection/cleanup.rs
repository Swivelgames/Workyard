//! Remove only displaced or partial entries whose contents can still be accounted for.
use super::{conflict, tree};
mod removal;
use crate::{
	exists,
	relocation::{content::inventory, record::Content},
};
use std::{
	collections::BTreeMap,
	fs,
	io::{BufReader, Read},
	path::{Path, PathBuf},
};
use workyard_core::{Control, Problem};

pub(super) fn verify_pointer(directory: &Path, admin: &Path) -> Result<(), Problem> {
	let path = directory.join(".git");
	let metadata =
		fs::symlink_metadata(&path).map_err(|e| Problem::io("Inspect staged Git pointer", e))?;
	if !metadata.is_file()
		|| fs::read(path).map_err(|e| Problem::io("Read staged Git pointer", e))?
			!= tree::pointer(admin)
	{
		return Err(conflict(
			"Recovered worktree pointer changed before publication.",
		));
	}
	Ok(())
}

pub(super) fn retired_inventory(path: &Path, control: &Control) -> Result<Vec<Content>, Problem> {
	inventory(path, Path::new(""), control)
}

pub(super) fn discard_retired(
	path: &Path,
	evidence: &[Content],
	control: &Control,
) -> Result<(), Problem> {
	let expected = by_path(evidence)?;
	let remaining = retired_inventory(path, control)?;
	for entry in &remaining {
		let rel = entry.path.decode()?;
		let Some(prior) = expected.get(&rel) else {
			return Err(conflict("Displaced files were added during recovery."));
		};
		if entry.kind != prior.kind
			|| entry.digest != prior.digest
			|| entry.link != prior.link
			|| entry.origin_identity != prior.origin_identity
			|| !removal::same_permissions(entry, prior)
		{
			return Err(conflict("Displaced files changed during recovery."));
		}
	}
	removal::verified_entries(path, &remaining, control)
}
fn by_path(entries: &[Content]) -> Result<BTreeMap<PathBuf, &Content>, Problem> {
	entries.iter().map(|e| Ok((e.path.decode()?, e))).collect()
}
pub(super) fn discard_partial(
	stage: &Path,
	source: &Path,
	admin: &Path,
	control: &Control,
) -> Result<(), Problem> {
	if exists(&stage.join(".git"))? {
		verify_pointer(stage, admin)?;
	}
	let content = inventory(source, &source.join(".git"), control)?;
	let expected = by_path(&content)?;
	let remaining = retired_inventory(stage, control)?;
	for entry in &remaining {
		let rel = entry.path.decode()?;
		if rel == Path::new(".git") {
			verify_pointer(stage, admin)?;
			continue;
		}
		let Some(source_entry) = expected.get(&rel) else {
			return Err(conflict("Recovery staging contains additional files."));
		};
		if entry.kind != source_entry.kind || entry.link != source_entry.link {
			return Err(conflict("Recovery staging contains a changed entry."));
		}
		if entry.kind == "file"
			&& entry.digest != source_entry.digest
			&& !is_file_prefix(&stage.join(&rel), &source.join(rel), control)?
		{
			return Err(conflict("Recovery staging contains edited files."));
		}
	}
	removal::verified_entries(stage, &remaining, control)
}

fn is_file_prefix(prefix: &Path, source: &Path, control: &Control) -> Result<bool, Problem> {
	let open = |p: &Path| {
		fs::File::open(p)
			.map(BufReader::new)
			.map_err(|e| Problem::io("Inspect interrupted staged file", e))
	};
	let mut partial = open(prefix)?;
	let mut original = open(source)?;
	let mut a = [0u8; 65536];
	let mut b = [0u8; 65536];
	loop {
		control.check()?;
		let n = partial
			.read(&mut a)
			.map_err(|e| Problem::io("Read interrupted staged file", e))?;
		if n == 0 {
			return Ok(true);
		}
		match original.read_exact(&mut b[..n]) {
			Ok(()) if a[..n] == b[..n] => (),
			Ok(()) => return Ok(false),
			Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => return Ok(false),
			Err(e) => return Err(Problem::io("Read original participant file", e)),
		}
	}
}
