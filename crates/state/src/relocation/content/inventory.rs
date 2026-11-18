//! Snapshot workspace content without following symlinks or entering state.
use super::digest;
use crate::relocation::record::Content;
use std::{fs, path::Path};
use workyard_core::{Control, Problem, ProblemCode};

pub(in super::super) fn inventory(
	root: &Path,
	state: &Path,
	control: &Control,
) -> Result<Vec<Content>, Problem> {
	let mut content = Vec::new();
	visit(root, root, state, &mut content, control)?;
	Ok(content)
}

fn visit(
	root: &Path,
	directory: &Path,
	state: &Path,
	content: &mut Vec<Content>,
	control: &Control,
) -> Result<(), Problem> {
	for entry in
		fs::read_dir(directory).map_err(|error| Problem::io("Read workspace contents", error))?
	{
		control.check()?;
		let entry = entry.map_err(|error| Problem::io("Read workspace entry", error))?;
		let path = entry.path();
		if path == state {
			continue;
		}
		let kind = entry
			.file_type()
			.map_err(|error| Problem::io("Inspect workspace entry", error))?;
		let (kind, digest, link) = if kind.is_dir() {
			("directory", String::new(), None)
		} else if kind.is_file() {
			("file", digest(&path, control)?, None)
		} else if kind.is_symlink() {
			let target = fs::read_link(&path)
				.map_err(|error| Problem::io("Read workspace symlink", error))?;
			("symlink", String::new(), Some(target.as_path().into()))
		} else {
			return Err(Problem::new(
				ProblemCode::InvalidInput,
				format!("Workspace contains an unsupported special file: {path:?}."),
			));
		};
		let metadata = fs::symlink_metadata(&path)
			.map_err(|error| Problem::io("Read workspace permissions", error))?;
		#[cfg(unix)]
		let mode = {
			use std::os::unix::fs::PermissionsExt;
			Some(metadata.permissions().mode())
		};
		#[cfg(not(unix))]
		let mode = None;
		content.push(Content {
			path: path
				.strip_prefix(root)
				.expect("workspace descendant")
				.into(),
			kind: kind.into(),
			digest,
			link,
			copied: false,
			origin_identity: Some(workyard_system::entry_identity(&path)?),
			destination_identity: None,
			staged_identity: None,
			staged_for_undo: None,
			mode,
			readonly: metadata.permissions().readonly(),
		});
		if kind == "directory" {
			visit(root, &path, state, content, control)?;
		}
	}
	Ok(())
}
