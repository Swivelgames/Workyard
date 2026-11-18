//! Stage regular file copies with a journaled identity and a verified source prefix.
use super::{conflict, digest, exists_entry};
use crate::relocation::record::Journal;
use std::{
	fs,
	io::{Read, Seek, SeekFrom, Write},
	path::{Path, PathBuf},
};
use workyard_core::{Control, Problem};

pub(super) fn temporary(destination: &Path, journal: &Journal, index: usize) -> PathBuf {
	destination
		.parent()
		.expect("content parent")
		.join(format!(".workyard-transfer-{}-{index}", journal.id))
}

pub(super) fn file(
	source: &Path,
	destination: &Path,
	journal: &mut Journal,
	index: usize,
	control: &Control,
	publish: &mut impl FnMut(&Journal) -> Result<(), Problem>,
) -> Result<(), Problem> {
	let temporary = temporary(destination, journal, index);
	let mut input =
		fs::File::open(source).map_err(|error| Problem::io("Open file copy source", error))?;
	let mut options = fs::OpenOptions::new();
	options.read(true).write(true).create_new(true);
	#[cfg(unix)]
	{
		use std::os::unix::fs::OpenOptionsExt;
		options.mode(0o600);
	}
	let mut output = match options.open(&temporary) {
		Ok(output) => {
			journal.content[index].staged_identity = Some(workyard_system::file_identity(&output)?);
			publish(journal)?;
			output
		}
		Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
			let metadata = fs::symlink_metadata(&temporary)
				.map_err(|error| Problem::io("Inspect staged transfer kind", error))?;
			if !metadata.is_file() {
				return Err(conflict(&temporary));
			}
			if metadata.permissions().readonly() {
				// An interruption after applying the source permissions can leave a
				// complete stage that cannot be reopened for writing.
				if metadata.permissions()
					!= input
						.metadata()
						.map_err(|error| Problem::io("Inspect transfer source permissions", error))?
						.permissions()
				{
					return Err(conflict(&temporary));
				}
				return publish_file(source, destination, journal, index, control, publish);
			}
			let output = fs::OpenOptions::new()
				.read(true)
				.write(true)
				.open(&temporary)
				.map_err(|error| Problem::io("Open pending transfer", error))?;
			if journal.content[index].staged_identity.as_ref()
				!= Some(&workyard_system::file_identity(&output)?)
			{
				return Err(conflict(&temporary));
			}
			output
		}
		Err(error) => return Err(Problem::io("Create pending transfer", error)),
	};
	verify_prefix(&mut input, &mut output, control)?;
	let mut buffer = [0; 65536];
	loop {
		control.check()?;
		let length = input
			.read(&mut buffer)
			.map_err(|error| Problem::io("Read cross-volume source", error))?;
		if length == 0 {
			break;
		}
		output
			.write_all(&buffer[..length])
			.map_err(|error| Problem::io("Write pending transfer", error))?;
	}
	output
		.sync_all()
		.map_err(|error| Problem::io("Flush pending transfer", error))?;
	output
		.set_permissions(
			input
				.metadata()
				.map_err(|error| Problem::io("Read source permissions", error))?
				.permissions(),
		)
		.map_err(|error| Problem::io("Preserve transfer permissions", error))?;
	publish_file(source, destination, journal, index, control, publish)
}

fn publish_file(
	source: &Path,
	destination: &Path,
	journal: &mut Journal,
	index: usize,
	control: &Control,
	publish: &mut impl FnMut(&Journal) -> Result<(), Problem>,
) -> Result<(), Problem> {
	let temporary = temporary(destination, journal, index);
	if Some(workyard_system::entry_identity(&temporary)?) != journal.content[index].staged_identity
	{
		return Err(conflict(&temporary));
	}
	if digest(&temporary, control)? != journal.content[index].digest
		|| digest(source, control)? != journal.content[index].digest
	{
		return Err(conflict(&temporary));
	}
	if exists_entry(destination)? {
		return Err(conflict(destination));
	}
	journal.content[index].destination_identity = journal.content[index].staged_identity;
	publish(journal)?;
	workyard_system::rename_no_replace(&temporary, destination)
		.map_err(|error| Problem::io("Publish verified file transfer", error))?;
	Ok(())
}

pub(super) fn discard_partial(
	source: &Path,
	destination: &Path,
	journal: &Journal,
	index: usize,
	control: &Control,
) -> Result<(), Problem> {
	let temporary = temporary(destination, journal, index);
	if !temporary
		.try_exists()
		.map_err(|error| Problem::io("Inspect partial transfer", error))?
	{
		return Ok(());
	}
	let mut input = fs::File::open(source)
		.map_err(|error| Problem::io("Verify partial transfer source", error))?;
	let mut output = fs::File::open(&temporary)
		.map_err(|error| Problem::io("Verify partial transfer", error))?;
	if journal.content[index].staged_identity.as_ref()
		!= Some(&workyard_system::file_identity(&output)?)
	{
		return Err(conflict(&temporary));
	}
	verify_prefix(&mut input, &mut output, control)?;
	drop(output);
	fs::remove_file(&temporary)
		.map_err(|error| Problem::io("Remove verified partial transfer for undo", error))
}

fn verify_prefix(
	input: &mut fs::File,
	output: &mut fs::File,
	control: &Control,
) -> Result<(), Problem> {
	let length = output
		.metadata()
		.map_err(|error| Problem::io("Inspect partial file length", error))?
		.len();
	let mut remaining = length;
	let mut source = [0; 65536];
	let mut target = [0; 65536];
	while remaining > 0 {
		control.check()?;
		let count = remaining.min(source.len() as u64) as usize;
		input
			.read_exact(&mut source[..count])
			.map_err(|error| Problem::io("Verify source prefix", error))?;
		output
			.read_exact(&mut target[..count])
			.map_err(|error| Problem::io("Verify pending prefix", error))?;
		if source[..count] != target[..count] {
			return Err(Problem::new(
				workyard_core::ProblemCode::RepositoryChanged,
				"Pending transfer was edited; it will not be overwritten.",
			));
		}
		remaining -= count as u64;
	}
	input
		.seek(SeekFrom::Start(length))
		.map_err(|error| Problem::io("Resume source stream", error))?;
	output
		.seek(SeekFrom::Start(length))
		.map_err(|error| Problem::io("Resume destination stream", error))?;
	Ok(())
}
