//! Copy the admitted inventory without ever renaming or unlinking source content.
pub(in crate::relocation) mod permissions;
mod verify;
use super::{conflict, copy, copy_symlink, entries, exists_entry, set_directory_permissions};
use crate::relocation::record::Journal;
use std::{
	collections::BTreeMap,
	fs,
	path::{Path, PathBuf},
};
use verify::{Endpoint, require_directory, verify_entry};
use workyard_core::{Control, Problem};

type Identity = (u64, u64);

pub(in super::super) fn copy_workspace(
	journal: &mut Journal,
	undo: bool,
	control: &Control,
	mut publish: impl FnMut(&Journal) -> Result<(), Problem>,
) -> Result<(), Problem> {
	let source = journal.old_root.decode()?;
	let destination = journal.new_root.decode()?;
	let gitfiles = verify::participant_gitfiles(journal, &source)?;

	if undo {
		// The source never moved. Git will independently validate its administrative
		// pointers; directory identity protects against a replacement with a copied pointer.
		verify::verify_worktree_directories(journal, Endpoint::Source, &gitfiles, control)?;
		if journal.copy_verified {
			verify::verify_worktree_directories(
				journal,
				Endpoint::Destination,
				&gitfiles,
				control,
			)?;
		}
		return Ok(());
	}
	require_directory(&source)?;
	if exists_entry(&destination)? {
		require_directory(&destination)?;
	} else {
		permissions::prepare_root(&destination)?;
	}

	if !journal.copy_verified {
		copy_entries(journal, &source, &destination, control, &mut publish)?;
	}
	verify::verify_tree(journal, Endpoint::Source, &gitfiles, control)?;
	verify::verify_tree(journal, Endpoint::Destination, &gitfiles, control)?;
	permissions::finish_root(&source, &destination, journal.copy_verified)?;
	if !journal.copy_verified {
		journal.copy_verified = true;
		publish(journal)?;
	}
	Ok(())
}

fn copy_entries(
	journal: &mut Journal,
	source: &Path,
	destination: &Path,
	control: &Control,
	publish: &mut impl FnMut(&Journal) -> Result<(), Problem>,
) -> Result<(), Problem> {
	let recovery = verify::recovery_paths(journal)?;
	let mut files: BTreeMap<Identity, (PathBuf, Identity)> = BTreeMap::new();
	for index in 0..journal.content.len() {
		control.check()?;
		let entry = journal.content[index].clone();
		let relative = entry.path.decode()?;
		let from = source.join(&relative);
		let to = destination.join(&relative);
		if verify::covered(&recovery, &from) || verify::covered(&recovery, &to) {
			continue;
		}
		verify_entry(&from, &entry, entry.origin_identity, control)?;
		permissions::verify_mode(&from, &entry, false)?;

		if exists_entry(&to)? {
			verify_entry(&to, &entry, entry.destination_identity, control)?;
			permissions::verify_mode(&to, &entry, true)?;
		} else if entry.copied {
			return Err(conflict(&to));
		} else {
			publish_copy(&from, &to, journal, index, &files, control, publish)?;
		}

		if entry.kind == "file" {
			let origin = entry.origin_identity.ok_or_else(|| conflict(&from))?;
			let id = journal.content[index]
				.destination_identity
				.ok_or_else(|| conflict(&to))?;
			if id == origin {
				return Err(conflict(&to));
			}
			files.entry(origin).or_insert((to, id));
		}
		if !entry.copied {
			journal.content[index].copied = true;
			publish(journal)?;
		}
	}
	for entry in journal
		.content
		.iter()
		.rev()
		.filter(|entry| entry.kind == "directory")
	{
		control.check()?;
		let target = destination.join(entry.path.decode()?);
		if verify::covered(&recovery, &target) {
			continue;
		}
		verify_entry(&target, entry, entry.destination_identity, control)?;
		permissions::verify_mode(&target, entry, true)?;
		set_directory_permissions(&target, entry)?;
	}
	Ok(())
}

fn publish_copy(
	from: &Path,
	to: &Path,
	journal: &mut Journal,
	index: usize,
	files: &BTreeMap<Identity, (PathBuf, Identity)>,
	control: &Control,
	publish: &mut impl FnMut(&Journal) -> Result<(), Problem>,
) -> Result<(), Problem> {
	let entry = journal.content[index].clone();
	if entry.kind != "file" {
		return entries::publish_entry(
			to,
			journal,
			index,
			false,
			|stage| {
				if entry.kind == "symlink" {
					return copy_symlink(from, stage, &entry);
				}
				permissions::private_directories()
					.recursive(false)
					.create(stage)
					.map_err(|error| Problem::io("Stage workspace copy directory", error))
			},
			publish,
		);
	}

	let origin = entry.origin_identity.ok_or_else(|| conflict(from))?;
	let Some((prior, identity)) = files.get(&origin) else {
		return copy::file(from, to, journal, index, control, publish);
	};

	// A crash immediately after creating a hardlink still leaves proof that
	// this destination name belongs to the same independently copied file.
	verify_entry(prior, &entry, Some(*identity), control)?;
	journal.content[index].destination_identity = Some(*identity);
	publish(journal)?;
	fs::hard_link(prior, to).map_err(|error| Problem::io("Link copied workspace files", error))
}

#[cfg(test)]
mod tests;
