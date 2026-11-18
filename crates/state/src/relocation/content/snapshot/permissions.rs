//! Keep copy staging private until admitted file and directory modes are restored.
use super::super::conflict;
use crate::relocation::record::Content;
use std::{fs, path::Path};
use workyard_core::Problem;

pub(in crate::relocation) fn private_directories() -> fs::DirBuilder {
	let mut builder = fs::DirBuilder::new();
	#[cfg(unix)]
	{
		use std::os::unix::fs::DirBuilderExt;
		builder.mode(0o700);
	}
	builder.recursive(true);
	builder
}

pub(in crate::relocation) fn prepare_root(root: &Path) -> Result<(), Problem> {
	private_directories()
		.create(root)
		.map_err(|error| Problem::io("Create workspace copy root", error))?;
	super::verify::require_directory(root)?;
	#[cfg(unix)]
	{
		use std::os::unix::fs::PermissionsExt;
		fs::set_permissions(root, fs::Permissions::from_mode(0o700))
			.map_err(|error| Problem::io("Protect workspace copy root", error))?;
	}
	Ok(())
}

pub(super) fn mode(path: &Path) -> Result<fs::Permissions, Problem> {
	fs::symlink_metadata(path)
		.map(|metadata| metadata.permissions())
		.map_err(|error| Problem::io("Inspect copied permissions", error))
}

fn staged_directory(permissions: &fs::Permissions) -> bool {
	#[cfg(unix)]
	{
		use std::os::unix::fs::PermissionsExt;
		permissions.mode() & 0o7777 == 0o700
	}
	#[cfg(not(unix))]
	{
		!permissions.readonly()
	}
}

pub(super) fn verify_mode(
	path: &Path,
	entry: &Content,
	allow_staged_directory: bool,
) -> Result<(), Problem> {
	if entry.kind == "symlink" {
		return Ok(());
	}
	let permissions = mode(path)?;
	#[cfg(unix)]
	let matches = {
		use std::os::unix::fs::PermissionsExt;
		entry.mode == Some(permissions.mode())
	};
	#[cfg(not(unix))]
	let matches = entry.readonly == permissions.readonly();
	if matches
		|| (allow_staged_directory && entry.kind == "directory" && staged_directory(&permissions))
	{
		return Ok(());
	}
	Err(conflict(path))
}

pub(super) fn finish_root(
	source: &Path,
	destination: &Path,
	verified: bool,
) -> Result<(), Problem> {
	let expected = mode(source)?;
	let actual = mode(destination)?;
	if actual == expected {
		return Ok(());
	}
	if verified || !staged_directory(&actual) {
		return Err(conflict(destination));
	}
	fs::set_permissions(destination, expected)
		.map_err(|error| Problem::io("Preserve workspace copy root permissions", error))
}
