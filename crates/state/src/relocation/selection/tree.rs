//! Fingerprint authored contents independently of Git's index and worktree pointer.
use super::{conflict, read_directory};
use crate::relocation::content::{copy_symlink, inventory, permissions};
use sha2::{Digest, Sha256};
use std::{collections::BTreeMap, fs, path::Path};
use workyard_core::{Control, Problem};

pub(super) fn fingerprint(root: &Path, control: &Control) -> Result<String, Problem> {
	let root_meta = read_directory(root)?;
	let mut hash = Sha256::new();
	#[cfg(unix)]
	{
		use std::os::unix::fs::PermissionsExt;
		hash.update(root_meta.permissions().mode().to_le_bytes());
	}
	hash.update([root_meta.permissions().readonly() as u8]);
	let mut entries: Vec<_> = inventory(root, &root.join(".git"), control)?
		.into_iter()
		.map(|entry| Ok((entry.path.decode()?, entry)))
		.collect::<Result<_, Problem>>()?;
	entries.sort_by(|a, b| a.0.cmp(&b.0));
	let mut first_link = BTreeMap::new();
	for (path, entry) in entries {
		let bytes = path.as_os_str().as_encoded_bytes();
		hash.update((bytes.len() as u64).to_le_bytes());
		hash.update(bytes);
		hash.update(entry.kind.as_bytes());
		hash.update(entry.mode.unwrap_or_default().to_le_bytes());
		hash.update([entry.readonly as u8]);
		hash.update(entry.digest.as_bytes());
		if entry.kind == "file" {
			// A representative name describes a link group across independent copies.
			let representative = first_link.entry(entry.origin_identity).or_insert(path);
			let name = representative.as_os_str().as_encoded_bytes();
			hash.update((name.len() as u64).to_le_bytes());
			hash.update(name);
		}
		if let Some(link) = entry.link {
			let target = link.decode()?;
			let bytes = target.as_os_str().as_encoded_bytes();
			hash.update((bytes.len() as u64).to_le_bytes());
			hash.update(bytes);
		}
	}
	Ok(hash
		.finalize()
		.iter()
		.map(|byte| format!("{byte:02x}"))
		.collect())
}

/// Stage under a private directory. Only the completed tree is ever published.
pub(super) fn populate(
	source: &Path,
	stage: &Path,
	admin: &Path,
	control: &Control,
) -> Result<(), Problem> {
	let content = inventory(source, &source.join(".git"), control)?;
	let mut links = BTreeMap::new();
	for entry in &content {
		control.check()?;
		let rel = entry.path.decode()?;
		let from = source.join(&rel);
		let to = stage.join(&rel);
		match entry.kind.as_str() {
			"directory" => permissions::private_directories()
				.recursive(false)
				.create(&to)
				.map_err(|e| Problem::io("Stage participant directory", e))?,
			"symlink" => copy_symlink(&from, &to, entry)?,
			"file" => {
				if let Some(prior) = links.get(&entry.origin_identity) {
					fs::hard_link(prior, &to)
						.map_err(|e| Problem::io("Stage participant hard link", e))?;
				} else {
					fs::copy(&from, &to).map_err(|e| Problem::io("Stage participant file", e))?;
					links.insert(entry.origin_identity, to);
				}
			}
			_ => return Err(conflict("Unsupported participant entry.")),
		}
	}
	fs::write(stage.join(".git"), pointer(admin))
		.map_err(|e| Problem::io("Stage verified worktree pointer", e))?;
	for entry in content.iter().rev().filter(|e| e.kind == "directory") {
		let relative = entry.path.decode()?;
		copy_mode(&source.join(&relative), &stage.join(relative))?;
	}
	copy_mode(source, stage)
}

fn copy_mode(source: &Path, target: &Path) -> Result<(), Problem> {
	let permissions = read_directory(source)?.permissions();
	fs::set_permissions(target, permissions)
		.map_err(|e| Problem::io("Preserve participant permissions", e))
}

pub(super) fn pointer(admin: &Path) -> Vec<u8> {
	let mut pointer = b"gitdir: ".to_vec();
	pointer.extend_from_slice(admin.as_os_str().as_encoded_bytes());
	pointer.push(b'\n');
	pointer
}
