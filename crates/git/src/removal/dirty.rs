use crate::{Runner, process::args};
use sha2::{Digest, Sha256};
use std::{
	io::Read,
	path::{Path, PathBuf},
};
use workyard_core::{Control, Problem};

/// Include file contents, not only porcelain status: editing an already-dirty
/// file while a prompt is visible requires a fresh decision too.
pub(super) async fn inspect(
	runner: &Runner,
	destination: &Path,
	control: &Control,
) -> Result<(Vec<PathBuf>, Vec<u8>), Problem> {
	let status = runner
		.checked(
			destination,
			&args(&[
				"--no-optional-locks",
				"status",
				"--porcelain=v1",
				"-z",
				"--untracked-files=all",
				"--ignored=matching",
			]),
			control,
		)
		.await?;
	let mut paths = Vec::new();
	let mut fields = status
		.split(|byte| *byte == 0)
		.filter(|field| !field.is_empty());
	while let Some(field) = fields.next() {
		if field.len() < 4 {
			return Err(super::blocked("Invalid Git status output."));
		}
		let path = crate::output::path_field(&field[3..])
			.map_err(|_| super::blocked("Invalid dirty-file path."))?;
		paths.push(path);
		if field[..2]
			.iter()
			.any(|status| matches!(status, b'R' | b'C'))
		{
			fields
				.next()
				.ok_or_else(|| super::blocked("Missing rename source in Git status output."))?;
		}
	}

	let mut hash = Sha256::new();
	frame(&mut hash, &status);
	let index = runner
		.checked(destination, &args(&["ls-files", "--stage", "-z"]), control)
		.await?;
	frame(&mut hash, &index);
	for path in &paths {
		control.check()?;
		frame(&mut hash, path.as_os_str().as_encoded_bytes());
		let mut entry = Sha256::new();
		hash_path(&destination.join(path), &mut entry, control)?;
		frame(&mut hash, &entry.finalize());
	}
	Ok((paths, hash.finalize().to_vec()))
}

fn hash_path(path: &Path, hash: &mut Sha256, control: &Control) -> Result<(), Problem> {
	control.check()?;
	let metadata = match std::fs::symlink_metadata(path) {
		Ok(metadata) => metadata,
		Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
			frame(hash, b"missing");
			return Ok(());
		}
		Err(error) => return Err(Problem::io("Inspect disclosed file", error)),
	};
	if metadata.file_type().is_symlink() {
		let target =
			std::fs::read_link(path).map_err(|e| Problem::io("Inspect disclosed symlink", e))?;
		frame(hash, b"symlink");
		frame(hash, target.as_os_str().as_encoded_bytes());
		return Ok(());
	}
	if metadata.is_dir() {
		frame(hash, b"directory");
		let mut children = std::fs::read_dir(path)
			.map_err(|e| Problem::io("Inspect disclosed directory", e))?
			.collect::<Result<Vec<_>, _>>()
			.map_err(|e| Problem::io("Read disclosed directory", e))?;
		children.sort_by_key(|child| child.file_name());
		for child in children {
			frame(hash, child.file_name().as_encoded_bytes());
			let mut entry = Sha256::new();
			hash_path(&child.path(), &mut entry, control)?;
			frame(hash, &entry.finalize());
		}
		return Ok(());
	}
	if !metadata.is_file() {
		return Err(super::blocked(
			"Cannot safely inspect a special file before removal.",
		));
	}
	frame(hash, b"file");
	frame(hash, &metadata.len().to_le_bytes());
	let mut file = std::fs::File::open(path).map_err(|e| Problem::io("Read disclosed file", e))?;
	let mut buffer = [0; 8192];
	loop {
		control.check()?;
		let size = file
			.read(&mut buffer)
			.map_err(|e| Problem::io("Read disclosed file", e))?;
		if size == 0 {
			break;
		}
		hash.update(&buffer[..size]);
	}
	Ok(())
}

fn frame(hash: &mut Sha256, bytes: &[u8]) {
	hash.update((bytes.len() as u64).to_le_bytes());
	hash.update(bytes);
}
