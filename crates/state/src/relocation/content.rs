//! Inventory and restartable native file transfer; source survives verified copying.
mod copy;
mod entries;
mod inventory;
mod snapshot;
use super::record::{Content, Journal};
pub(super) use entries::remove_symlink;
pub(super) use inventory::inventory;
use sha2::{Digest, Sha256};
pub(super) use snapshot::{copy_workspace, permissions};
use std::{fs, io::Read, path::Path};
use workyard_core::{Control, Problem, ProblemCode};

pub(super) fn transfer(
	journal: &mut Journal,
	undo: bool,
	control: &Control,
	mut publish: impl FnMut(&Journal) -> Result<(), Problem>,
) -> Result<(), Problem> {
	let old = journal.old_root.decode()?;
	let new = journal.new_root.decode()?;
	if old == new {
		return Ok(());
	}
	let (source, destination) = if undo { (&new, &old) } else { (&old, &new) };
	entries::verify(journal, &old, &new, control)?;
	entries::discard_stages(source, destination, journal, undo, control)?;
	fs::create_dir_all(destination)
		.map_err(|error| Problem::io("Create workspace transfer root", error))?;
	for index in 0..journal.content.len() {
		control.check()?;
		let entry = journal.content[index].clone();
		let relative = entry.path.decode()?;
		let from = source.join(&relative);
		let to = destination.join(&relative);
		if undo && entry.staged_identity.is_some() && entry.kind == "file" {
			copy::discard_partial(
				&old.join(&relative),
				&new.join(&relative),
				journal,
				index,
				control,
			)?;
		}
		if entry.kind == "directory" {
			let from_exists = entries::verify_existing(&from, &entry)?;
			if entries::verify_existing(&to, &entry)? {
				continue;
			}
			if !from_exists {
				return Err(conflict(&from));
			}

			// Keep an intact directory on the same volume. A directory that
			// contains source state must stay until the authority handoff.
			let protected = if undo {
				journal.new_state.decode()?
			} else {
				journal.old_state.decode()?
			};
			if !protected.starts_with(&from) {
				match workyard_system::rename_no_replace(&from, &to) {
					Ok(()) => {
						for moved in &mut journal.content {
							if moved.path.decode()?.starts_with(&relative) {
								moved.copied = true;
							}
						}
						publish(journal)?;
						continue;
					}
					Err(error) if error.kind() == std::io::ErrorKind::CrossesDevices => {}
					Err(error) => {
						return Err(Problem::io("Move workspace directory intact", error));
					}
				}
			}
			entries::publish_entry(
				&to,
				journal,
				index,
				undo,
				|stage| {
					fs::create_dir(stage)
						.map_err(|error| Problem::io("Stage relocated directory", error))
				},
				&mut publish,
			)?;
			continue;
		}
		let from_exists = exists_entry(&from)?;
		let to_exists = exists_entry(&to)?;
		if entry.kind == "symlink" {
			entries::verify_existing(&from, &entry)?;
			entries::verify_existing(&to, &entry)?;
		}
		if !from_exists {
			let owned = if to_exists && entry.kind == "file" {
				if !fs::symlink_metadata(&to)
					.map_err(|error| Problem::io("Inspect moved target kind", error))?
					.is_file()
				{
					return Err(conflict(&to));
				}
				let identity = workyard_system::file_identity(
					&fs::File::open(&to)
						.map_err(|error| Problem::io("Verify moved file identity", error))?,
				)?;
				Some(identity) == entry.origin_identity
					|| Some(identity) == entry.destination_identity
			} else {
				to_exists && matches(&to, &entry, control)?
			};
			if !owned {
				return Err(conflict(&to));
			}
			continue;
		}
		if to_exists {
			if entry.kind == "file" {
				let identity =
					workyard_system::file_identity(&fs::File::open(&to).map_err(|error| {
						Problem::io("Inspect existing transfer target", error)
					})?)?;
				let owned = Some(identity) == entry.origin_identity
					|| Some(identity) == entry.destination_identity
					|| journal.content[..index].iter().any(|prior| {
						prior.origin_identity == entry.origin_identity
							&& prior.destination_identity == Some(identity)
					});
				if !owned {
					return Err(conflict(&to));
				}
			}
			if !matches(&from, &entry, control)? || !matches(&to, &entry, control)? {
				return Err(conflict(&to));
			}
		} else {
			if !entry.copied && !matches(&from, &entry, control)? {
				return Err(conflict(&from));
			}
			if let Some(parent) = to.parent() {
				fs::create_dir_all(parent)
					.map_err(|error| Problem::io("Create transfer parent", error))?;
			}
			match workyard_system::rename_no_replace(&from, &to) {
				Ok(()) => {}
				Err(error) if error.kind() == std::io::ErrorKind::CrossesDevices => {
					if entry.kind == "file" {
						let prior = journal.content[..index].iter().find(|prior| {
							prior.origin_identity == entry.origin_identity && prior.kind == "file"
						});
						let linked = if let Some(prior) = prior {
							let path = destination.join(prior.path.decode()?);
							if path.try_exists().map_err(|error| {
								Problem::io("Inspect transferred hardlink", error)
							})? {
								fs::hard_link(path, &to).map_err(|error| {
									Problem::io("Preserve workspace hardlink", error)
								})?;
								true
							} else {
								false
							}
						} else {
							false
						};
						if !linked {
							copy::file(&from, &to, journal, index, control, &mut publish)?;
						}
					} else {
						entries::publish_entry(
							&to,
							journal,
							index,
							undo,
							|stage| copy_symlink(&from, stage, &entry),
							&mut publish,
						)?;
					}
					if !matches(&to, &entry, control)? || !matches(&from, &entry, control)? {
						return Err(conflict(&to));
					}
				}
				Err(error) => return Err(Problem::io("Move workspace file", error)),
			}
		}
		journal.content[index].copied = true;
		if entry.kind == "file" {
			journal.content[index].destination_identity = Some(workyard_system::file_identity(
				&fs::File::open(&to)
					.map_err(|error| Problem::io("Verify destination identity", error))?,
			)?);
		}
		publish(journal)?;
		if exists_entry(&from)? {
			if entry.kind == "symlink" {
				entries::verify_existing(&from, &journal.content[index])?;
			}
			if !matches(&from, &journal.content[index], control)? {
				return Err(conflict(&from));
			}
			let removed = if entry.kind == "symlink" {
				entries::remove_symlink(&from)
			} else {
				fs::remove_file(&from)
			};
			removed.map_err(|error| Problem::io("Remove verified source file", error))?;
		}
	}
	for entry in journal
		.content
		.iter()
		.rev()
		.filter(|entry| entry.kind == "directory")
	{
		let relative = entry.path.decode()?;
		let target = destination.join(&relative);
		if !entries::verify_existing(&target, entry)? {
			return Err(conflict(&target));
		}
		set_directory_permissions(&target, entry)?;
		let path = source.join(relative);
		if !entries::verify_existing(&path, entry)? {
			continue;
		}
		match fs::remove_dir(&path) {
			Ok(()) => {}
			Err(error)
				if matches!(
					error.kind(),
					std::io::ErrorKind::NotFound | std::io::ErrorKind::DirectoryNotEmpty
				) => {}
			Err(error) => return Err(Problem::io("Remove empty source directory", error)),
		}
	}
	Ok(())
}

fn set_directory_permissions(target: &Path, entry: &Content) -> Result<(), Problem> {
	let mut permissions = fs::metadata(target)
		.map_err(|error| Problem::io("Read relocated directory permissions", error))?
		.permissions();
	#[cfg(unix)]
	if let Some(mode) = entry.mode {
		use std::os::unix::fs::PermissionsExt;
		permissions.set_mode(mode);
	}
	#[cfg(not(unix))]
	permissions.set_readonly(entry.readonly);
	fs::set_permissions(target, permissions)
		.map_err(|error| Problem::io("Preserve directory permissions", error))?;
	Ok(())
}

pub(super) fn copy_symlink(
	_source: &Path,
	destination: &Path,
	entry: &Content,
) -> Result<(), Problem> {
	let target = entry.link.as_ref().expect("symlink target").decode()?;
	#[cfg(unix)]
	std::os::unix::fs::symlink(target, destination)
		.map_err(|error| Problem::io("Copy workspace symlink", error))?;
	#[cfg(windows)]
	{
		use std::os::windows::fs::FileTypeExt;

		let kind = _source
			.symlink_metadata()
			.map_err(|error| Problem::io("Inspect source symlink kind", error))?
			.file_type();
		if kind.is_symlink_dir() {
			std::os::windows::fs::symlink_dir(target, destination)
		} else {
			std::os::windows::fs::symlink_file(target, destination)
		}
		.map_err(|error| Problem::io("Copy workspace symlink", error))?;
	}
	Ok(())
}

pub(super) fn matches(path: &Path, entry: &Content, control: &Control) -> Result<bool, Problem> {
	let kind = fs::symlink_metadata(path)
		.map_err(|error| Problem::io("Inspect transfer file kind", error))?
		.file_type();
	if (entry.kind == "file" && !kind.is_file()) || (entry.kind == "symlink" && !kind.is_symlink())
	{
		return Ok(false);
	}
	if entry.kind == "symlink" {
		return Ok(fs::read_link(path)
			.map_err(|error| Problem::io("Verify transferred symlink", error))?
			== entry.link.as_ref().expect("symlink target").decode()?);
	}
	Ok(digest(path, control)? == entry.digest)
}

fn digest(path: &Path, control: &Control) -> Result<String, Problem> {
	let mut file =
		fs::File::open(path).map_err(|error| Problem::io("Open transfer file", error))?;
	let mut hash = Sha256::new();
	let mut buffer = [0; 65536];
	loop {
		control.check()?;
		let length = file
			.read(&mut buffer)
			.map_err(|error| Problem::io("Read transfer file", error))?;
		if length == 0 {
			break;
		}
		hash.update(&buffer[..length]);
	}
	Ok(hash
		.finalize()
		.iter()
		.map(|byte| format!("{byte:02x}"))
		.collect())
}

fn conflict(path: &Path) -> Problem {
	Problem::new(ProblemCode::RepositoryChanged, format!("Relocation content differs from verified transfer evidence: {path:?}."))
		.remedy("Preserve both locations and inspect the changed content before continuing. No existing file was overwritten.")
}

fn exists_entry(path: &Path) -> Result<bool, Problem> {
	match fs::symlink_metadata(path) {
		Ok(_) => Ok(true),
		Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
		Err(error) => Err(Problem::io("Inspect relocation entry", error)),
	}
}
