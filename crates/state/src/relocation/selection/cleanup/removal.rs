//! Unlink only entries verified in this pass. Unknown children keep their parent alive.
use super::conflict;
use crate::relocation::{
	content::{matches, remove_symlink},
	record::Content,
};
use std::{fs, path::Path};
use workyard_core::{Control, Problem};
use workyard_system::entry_identity;

pub(super) fn same_permissions(actual: &Content, expected: &Content) -> bool {
	if actual.mode == expected.mode && actual.readonly == expected.readonly {
		return true;
	}
	// A previous pass may already have made a verified directory removable.
	#[cfg(unix)]
	if actual.kind == "directory" {
		return actual.mode == expected.mode.map(|mode| mode | 0o300);
	}
	false
}

pub(super) fn verified_entries(
	root: &Path,
	entries: &[Content],
	control: &Control,
) -> Result<(), Problem> {
	// Inventory is parent-first. Make verified directories writable before unlinking
	// children, then retire them in reverse order using empty-directory removal.
	#[cfg(unix)]
	{
		writable_directory(root)?;
		for entry in entries.iter().filter(|entry| entry.kind == "directory") {
			control.check()?;
			let path = root.join(entry.path.decode()?);
			check_identity(&path, entry)?;
			writable_directory(&path)?;
		}
	}
	for entry in entries.iter().rev() {
		control.check()?;
		let path = root.join(entry.path.decode()?);
		check_identity(&path, entry)?;
		if entry.kind != "directory" && !matches(&path, entry, control)? {
			return Err(conflict(&format!("Recovery contents changed at {path:?}.")));
		}
		let removed = match entry.kind.as_str() {
			"directory" => fs::remove_dir(&path),
			"symlink" => remove_symlink(&path),
			_ => fs::remove_file(&path),
		};
		removed.map_err(|error| Problem::io(format!("Remove recovery entry {path:?}"), error))?;
	}
	fs::remove_dir(root)
		.map_err(|error| Problem::io(format!("Remove empty recovery directory {root:?}"), error))
}

fn check_identity(path: &Path, entry: &Content) -> Result<(), Problem> {
	if Some(entry_identity(path)?) != entry.origin_identity {
		return Err(conflict(&format!(
			"Recovery entry was replaced at {path:?}."
		)));
	}
	Ok(())
}

#[cfg(unix)]
fn writable_directory(path: &Path) -> Result<(), Problem> {
	use std::os::unix::fs::PermissionsExt;

	let metadata = super::super::read_directory(path)?;
	let mode = metadata.permissions().mode();
	if mode & 0o300 == 0o300 {
		return Ok(());
	}
	fs::set_permissions(path, fs::Permissions::from_mode(mode | 0o300)).map_err(|error| {
		Problem::io(
			format!("Prepare recovery directory {path:?} for removal"),
			error,
		)
	})
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn late_addition_survives_cleanup() {
		let fixture = tempfile::tempdir().unwrap();
		let root = fixture.path().join("displaced");
		fs::create_dir(&root).unwrap();
		fs::write(root.join("known"), "approved overwrite").unwrap();
		let control = Control::default();
		let entries = super::super::retired_inventory(&root, &control).unwrap();
		fs::write(root.join("late"), "new work").unwrap();

		assert!(verified_entries(&root, &entries, &control).is_err());
		assert_eq!(fs::read(root.join("late")).unwrap(), b"new work");
	}
}
