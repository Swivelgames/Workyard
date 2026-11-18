//! Verify copy contents and worktree identities around Git repair.
use super::Identity;
use crate::relocation::{
	content::{conflict, exists_entry, matches},
	record::{Content, Journal},
};
use std::{
	collections::BTreeSet,
	fs,
	path::{Path, PathBuf},
};
use workyard_core::{Control, Problem};
use workyard_system::entry_identity;

#[derive(Clone, Copy)]
pub(super) enum Endpoint {
	Source,
	Destination,
}

impl Endpoint {
	fn root(self, journal: &Journal) -> Result<PathBuf, Problem> {
		match self {
			Self::Source => journal.old_root.decode(),
			Self::Destination => journal.new_root.decode(),
		}
	}

	fn identity(self, entry: &Content) -> Option<Identity> {
		match self {
			Self::Source => entry.origin_identity,
			Self::Destination => entry.destination_identity,
		}
	}
}

/// These participants have their own live inventory, chosen after Git found the manual move.
pub(super) fn recovery_paths(journal: &Journal) -> Result<BTreeSet<PathBuf>, Problem> {
	let mut paths = BTreeSet::new();
	for (index, member) in journal.participants.iter().enumerate() {
		let Some(selection) = &member.selection else {
			continue;
		};
		paths.insert(
			member.task.participants[member.index]
				.destination
				.decode()?,
		);
		paths.insert(member.destination.decode()?);
		paths.extend(selection.scratch_paths(journal, index)?);
	}
	Ok(paths)
}

pub(super) fn covered(paths: &BTreeSet<PathBuf>, path: &Path) -> bool {
	path.ancestors().any(|ancestor| paths.contains(ancestor))
}

pub(super) fn verify_entry(
	path: &Path,
	entry: &Content,
	expected: Option<Identity>,
	control: &Control,
) -> Result<(), Problem> {
	if !exists_entry(path)? || Some(entry_identity(path)?) != expected {
		return Err(conflict(path));
	}
	if entry.kind == "directory" {
		return require_directory(path);
	}
	if !matches(path, entry, control)? {
		return Err(conflict(path));
	}
	Ok(())
}

pub(super) fn require_directory(path: &Path) -> Result<(), Problem> {
	if path
		.symlink_metadata()
		.map_err(|error| Problem::io("Inspect workspace directory", error))?
		.is_dir()
	{
		return Ok(());
	}
	Err(conflict(path))
}

pub(super) fn participant_gitfiles(
	journal: &Journal,
	source: &Path,
) -> Result<BTreeSet<PathBuf>, Problem> {
	let mut gitfiles = BTreeSet::new();
	for member in &journal.participants {
		let participant = &member.task.participants[member.index];
		if participant.worktree_identity.is_some()
			&& let Ok(relative) = participant.destination.decode()?.strip_prefix(source)
		{
			gitfiles.insert(relative.join(".git"));
		}
	}
	Ok(gitfiles)
}

pub(super) fn verify_worktree_directories(
	journal: &Journal,
	endpoint: Endpoint,
	gitfiles: &BTreeSet<PathBuf>,
	control: &Control,
) -> Result<(), Problem> {
	let root = endpoint.root(journal)?;
	if matches!(endpoint, Endpoint::Destination) && !exists_entry(&root)? {
		return Ok(());
	}
	require_directory(&root)?;
	let recovery = recovery_paths(journal)?;
	let directories: BTreeSet<_> = gitfiles
		.iter()
		.flat_map(|gitfile| gitfile.ancestors().skip(1))
		.collect();
	for entry in journal
		.content
		.iter()
		.filter(|entry| entry.kind == "directory")
	{
		control.check()?;
		let relative = entry.path.decode()?;
		if directories.contains(relative.as_path()) {
			let path = root.join(relative);
			if covered(&recovery, &path) {
				continue;
			}
			// Undo preserves destination files. Git independently verifies the
			// registered endpoint when a binding needs to move back.
			if matches!(endpoint, Endpoint::Destination) && !exists_entry(&path)? {
				continue;
			}
			verify_entry(&path, entry, endpoint.identity(entry), control)?;
		}
	}
	Ok(())
}

pub(super) fn verify_tree(
	journal: &Journal,
	endpoint: Endpoint,
	gitfiles: &BTreeSet<PathBuf>,
	control: &Control,
) -> Result<(), Problem> {
	let root = endpoint.root(journal)?;
	let state = match endpoint {
		Endpoint::Source => journal.old_state.decode()?,
		Endpoint::Destination => journal.new_state.decode()?,
	};
	let recovery = recovery_paths(journal)?;
	let mut expected = BTreeSet::new();
	let mut directories = vec![root.to_owned()];
	for entry in &journal.content {
		control.check()?;
		let relative = entry.path.decode()?;
		let path = root.join(&relative);
		if covered(&recovery, &path) {
			continue;
		}
		// Git may rewrite a worktree pointer while repairing registration.
		// The Git adapter verifies those relationships after content transfer.
		if !journal.copy_verified || !gitfiles.contains(&relative) {
			let identity = endpoint.identity(entry);
			verify_entry(&path, entry, identity, control)?;
			super::permissions::verify_mode(&path, entry, false)?;
		}
		if entry.kind == "directory" {
			directories.push(path.clone());
		}
		expected.insert(path);
	}
	while let Some(directory) = directories.pop() {
		for entry in fs::read_dir(directory)
			.map_err(|error| Problem::io("Verify workspace copy inventory", error))?
		{
			control.check()?;
			let path = entry
				.map_err(|error| Problem::io("Verify workspace copy entry", error))?
				.path();
			if expected.contains(&path) || path == state || covered(&recovery, &path) {
				continue;
			}
			// A nested custom state directory may need additional empty
			// ancestors. Inspect them for unrelated entries as well.
			if state.starts_with(&path) {
				require_directory(&path)?;
				directories.push(path);
				continue;
			}
			return Err(conflict(&path));
		}
	}
	Ok(())
}
