//! Ownership checks and staged publication for directories and symlinks.
use super::{conflict, copy, exists_entry};
use crate::relocation::record::{Content, Journal};
use std::{fs, path::Path};
use workyard_core::{Control, Problem};
use workyard_system::entry_identity;

pub(super) fn verify(
	journal: &Journal,
	old: &Path,
	new: &Path,
	control: &Control,
) -> Result<(), Problem> {
	for root in [old, new] {
		if exists_entry(root)?
			&& !root
				.symlink_metadata()
				.map_err(|error| Problem::io("Inspect workspace transfer root", error))?
				.is_dir()
		{
			return Err(conflict(root));
		}
	}
	for entry in journal.content.iter().filter(|entry| entry.kind != "file") {
		control.check()?;
		let relative = entry.path.decode()?;
		for root in [old, new] {
			verify_existing(&root.join(&relative), entry)?;
		}
	}
	Ok(())
}

pub(super) fn verify_existing(path: &Path, entry: &Content) -> Result<bool, Problem> {
	let kind = match path.symlink_metadata() {
		Ok(metadata) => metadata.file_type(),
		Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(false),
		Err(error) => return Err(Problem::io("Inspect transfer entry kind", error)),
	};
	if (entry.kind == "directory" && !kind.is_dir())
		|| (entry.kind == "symlink" && !kind.is_symlink())
	{
		return Err(conflict(path));
	}
	let identity = Some(entry_identity(path)?);
	if identity != entry.origin_identity && identity != entry.destination_identity {
		return Err(conflict(path));
	}
	Ok(true)
}

pub(super) fn publish_entry(
	target: &Path,
	journal: &mut Journal,
	index: usize,
	undo: bool,
	create: impl FnOnce(&Path) -> Result<(), Problem>,
	publish: &mut impl FnMut(&Journal) -> Result<(), Problem>,
) -> Result<(), Problem> {
	let stage = copy::temporary(target, journal, index);
	let entry = &mut journal.content[index];
	let identity = if undo {
		&mut entry.origin_identity
	} else {
		&mut entry.destination_identity
	};
	if exists_entry(&stage)? {
		if entry.staged_for_undo != Some(undo)
			|| Some(entry_identity(&stage)?) != entry.staged_identity
		{
			return Err(conflict(&stage));
		}
	} else {
		create(&stage)?;
		*identity = Some(entry_identity(&stage)?);
		entry.staged_identity = *identity;
		entry.staged_for_undo = Some(undo);
		publish(journal)?;
	}
	workyard_system::rename_no_replace(&stage, target)
		.map_err(|error| Problem::io("Publish staged workspace entry", error))
}

// On a direction change, discard only the verified, unpublished entry.
pub(super) fn discard_stages(
	source: &Path,
	destination: &Path,
	journal: &Journal,
	undo: bool,
	control: &Control,
) -> Result<(), Problem> {
	for (index, entry) in journal.content.iter().enumerate().rev() {
		control.check()?;
		if entry.kind != "file" {
			let relative = entry.path.decode()?;
			discard_stage(
				&source.join(&relative),
				&destination.join(&relative),
				journal,
				index,
				undo,
			)?;
		}
	}
	Ok(())
}

fn discard_stage(
	source: &Path,
	destination: &Path,
	journal: &Journal,
	index: usize,
	undo: bool,
) -> Result<(), Problem> {
	let stage = copy::temporary(source, journal, index);
	if !exists_entry(&stage)? {
		return Ok(());
	}
	let entry = &journal.content[index];
	if entry.staged_for_undo != Some(!undo)
		|| Some(entry_identity(&stage)?) != entry.staged_identity
		|| !verify_existing(destination, entry)?
	{
		return Err(conflict(&stage));
	}
	let removed = if entry.kind == "directory" {
		fs::remove_dir(&stage)
	} else {
		remove_symlink(&stage)
	};
	removed.map_err(|error| Problem::io("Discard staged workspace entry", error))
}

pub(super) fn remove_symlink(path: &Path) -> std::io::Result<()> {
	#[cfg(windows)]
	{
		use std::os::windows::fs::FileTypeExt;
		if path.symlink_metadata()?.file_type().is_symlink_dir() {
			return fs::remove_dir(path);
		}
	}
	fs::remove_file(path)
}
