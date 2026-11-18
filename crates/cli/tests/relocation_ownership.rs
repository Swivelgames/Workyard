#![cfg(unix)]

#[allow(dead_code)]
mod support;

use std::{
	fs,
	os::unix::fs::{MetadataExt, PermissionsExt, symlink},
	path::Path,
};
use support::{assert_ok, repo, text, workyard};
use workyard_core::{Control, RelocationState};
use workyard_state::Relocation;

#[test]
fn recovery_preserves_replacement_directories_and_symlinks() {
	check_replacements(false);
}

#[test]
fn cross_volume_recovery_preserves_replacement_directories_and_symlinks() {
	check_replacements(true);
}

fn check_replacements(cross_volume: bool) {
	let fixture = tempfile::tempdir().unwrap();
	let volume = Path::new("/dev/shm");
	let destination_parent = if cross_volume {
		let Ok(metadata) = volume.metadata() else {
			return;
		};
		if metadata.dev() == fixture.path().metadata().unwrap().dev() {
			return;
		}
		tempfile::tempdir_in(volume).unwrap()
	} else {
		tempfile::tempdir_in(fixture.path()).unwrap()
	};
	let old = fixture.path().join("old");
	let new = destination_parent.path().join("new");
	let state = old.join(".workyard");
	let source = repo(fixture.path(), "api");
	fs::create_dir(&old).unwrap();
	assert_ok(&workyard(
		&old,
		&[
			"create",
			"feat/one",
			"--repo",
			text(&source),
			"--from",
			"main",
			"--no-fetch",
		],
	));
	fs::create_dir(old.join("notes")).unwrap();
	symlink("notes", old.join("current")).unwrap();

	let mut session = Relocation::open(&state, Some(&old), &new, None).unwrap();
	session.begin(session.participants()).unwrap();
	session.transfer(false, &Control::default()).unwrap();
	drop(session);

	let directory = new.join("notes");
	let saved_directory = new.join("saved-notes");
	fs::rename(&directory, &saved_directory).unwrap();
	fs::create_dir(&directory).unwrap();
	fs::set_permissions(&directory, fs::Permissions::from_mode(0o700)).unwrap();
	fs::write(directory.join("authored"), "keep").unwrap();
	assert_blocked(fixture.path(), &state, "notes");
	assert_eq!(
		fs::read_to_string(directory.join("authored")).unwrap(),
		"keep"
	);
	assert_eq!(directory.metadata().unwrap().mode() & 0o777, 0o700);
	fs::rename(&directory, destination_parent.path().join("authored-notes")).unwrap();
	fs::rename(&saved_directory, &directory).unwrap();

	let link = new.join("current");
	let saved_link = new.join("saved-current");
	fs::rename(&link, &saved_link).unwrap();
	symlink("notes", &link).unwrap();
	let replacement = link.symlink_metadata().unwrap().ino();
	assert_blocked(fixture.path(), &state, "current");
	assert_eq!(link.symlink_metadata().unwrap().ino(), replacement);
	fs::remove_file(&link).unwrap();
	fs::rename(&saved_link, &link).unwrap();

	assert_ok(&workyard(
		fixture.path(),
		&["recover", "--state-dir", text(&state), "--undo"],
	));
	assert_eq!(old.join("current").read_link().unwrap(), Path::new("notes"));
	assert!(old.join("notes").is_dir());
}

fn assert_blocked(cwd: &Path, state: &Path, entry: &str) {
	for undo in [false, true] {
		let mut args = vec!["recover", "--state-dir", text(state)];
		if undo {
			args.push("--undo");
		}
		let output = workyard(cwd, &args);
		let rendered = format!(
			"{}{}",
			String::from_utf8_lossy(&output.stdout),
			String::from_utf8_lossy(&output.stderr)
		);
		assert!(!output.status.success(), "{rendered}");
		assert!(
			rendered.contains("differs from verified transfer evidence")
				&& rendered.contains(entry),
			"{rendered}"
		);
	}
}
