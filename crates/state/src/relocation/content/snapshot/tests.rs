use super::copy_workspace;
use crate::relocation::{content::inventory, record::Journal};
use std::{fs, path::Path};
use workyard_core::{Control, Problem, ProblemCode};
use workyard_system::entry_identity;

fn journal(source: &Path, target: &Path) -> Journal {
	let state = source.join(".workyard");
	Journal {
		version: 2,
		id: "snapshot-test".into(),
		copy: true,
		copy_verified: false,
		old_root: source.into(),
		new_root: target.into(),
		old_state: state.as_path().into(),
		new_state: target.join(".workyard").as_path().into(),
		tasks: Default::default(),
		participants: Vec::new(),
		files: Vec::new(),
		state_directories: Vec::new(),
		content: inventory(source, &state, &Control::default()).unwrap(),
		writer_pending: false,
		writer_description: None,
		activated: false,
		activation_pending: false,
		undone: false,
		cleanup_complete: false,
	}
}

#[test]
fn copy_resumes_verified_hardlinks_and_keeps_source_independent() {
	let source = tempfile::tempdir().unwrap();
	let target = tempfile::tempdir().unwrap();
	let file = source.path().join("notes");
	fs::write(&file, b"notes").unwrap();
	fs::hard_link(&file, source.path().join("alias")).unwrap();
	let mut journal = journal(source.path(), target.path());
	let control = Control::default();
	let stopped = copy_workspace(&mut journal, false, &control, |journal| {
		if journal.content.iter().any(|entry| entry.copied) {
			return Err(Problem::new(
				ProblemCode::Cancelled,
				"Simulated interruption",
			));
		}
		Ok(())
	});
	assert!(stopped.is_err());
	copy_workspace(&mut journal, false, &control, |_| Ok(())).unwrap();
	assert!(journal.copy_verified);
	let copied = target.path().join("notes");
	assert_ne!(
		entry_identity(&file).unwrap(),
		entry_identity(&copied).unwrap()
	);
	assert_eq!(
		entry_identity(&copied).unwrap(),
		entry_identity(&target.path().join("alias")).unwrap()
	);
	fs::write(&copied, b"authored edit").unwrap();
	assert!(copy_workspace(&mut journal, false, &control, |_| Ok(())).is_err());
	copy_workspace(&mut journal, true, &control, |_| Ok(())).unwrap();
	assert_eq!(fs::read(copied).unwrap(), b"authored edit");
	assert_eq!(fs::read(file).unwrap(), b"notes");
}

#[test]
fn foreign_destination_file_with_identical_bytes_is_preserved() {
	let source = tempfile::tempdir().unwrap();
	let target = tempfile::tempdir().unwrap();
	fs::write(source.path().join("notes"), b"notes").unwrap();
	let mut journal = journal(source.path(), target.path());
	let conflict = target.path().join("notes");
	fs::write(&conflict, b"notes").unwrap();
	let original = entry_identity(&conflict).unwrap();
	assert!(copy_workspace(&mut journal, false, &Control::default(), |_| Ok(())).is_err());
	assert_eq!(entry_identity(&conflict).unwrap(), original);
}

#[cfg(unix)]
#[test]
fn copy_retains_symlink_and_directory_modes() {
	use std::os::unix::fs::{PermissionsExt, symlink};

	let source = tempfile::tempdir().unwrap();
	let target = tempfile::tempdir().unwrap();
	let subdir = source.path().join("private");
	fs::create_dir(&subdir).unwrap();
	fs::set_permissions(&subdir, fs::Permissions::from_mode(0o750)).unwrap();
	symlink("missing", subdir.join("dangling")).unwrap();

	let mut journal = journal(source.path(), target.path());
	copy_workspace(&mut journal, false, &Control::default(), |_| Ok(())).unwrap();
	let copied = target.path().join("private");
	assert_eq!(
		copied.metadata().unwrap().permissions().mode() & 0o777,
		0o750
	);
	assert_eq!(
		copied.join("dangling").read_link().unwrap(),
		Path::new("missing")
	);
}

#[cfg(unix)]
#[test]
fn cross_volume_readonly_stage_resumes_after_identity_publication() {
	use std::os::unix::fs::{MetadataExt, PermissionsExt};

	let source = tempfile::tempdir().unwrap();
	let volume = Path::new("/dev/shm");
	let Ok(volume_metadata) = volume.metadata() else {
		return;
	};
	if source.path().metadata().unwrap().dev() == volume_metadata.dev() {
		return;
	}
	let target = tempfile::tempdir_in(volume).unwrap();
	let path = source.path().join("readonly");
	fs::write(&path, b"original").unwrap();
	fs::set_permissions(&path, fs::Permissions::from_mode(0o400)).unwrap();

	let mut journal = journal(source.path(), target.path());
	let control = Control::default();
	let stopped = copy_workspace(&mut journal, false, &control, |journal| {
		if journal.content[0].destination_identity.is_some() {
			return Err(Problem::new(ProblemCode::Cancelled, "Stop before publish"));
		}
		Ok(())
	});
	assert!(stopped.is_err());
	let copied = target.path().join("readonly");
	assert!(!copied.exists());

	copy_workspace(&mut journal, false, &control, |_| Ok(())).unwrap();
	assert_eq!(fs::read(&copied).unwrap(), b"original");
	assert_eq!(
		copied.metadata().unwrap().permissions().mode() & 0o777,
		0o400
	);
	assert_eq!(fs::read(path).unwrap(), b"original");
}

#[cfg(unix)]
#[test]
fn copy_stages_privately_and_detects_permission_edits_on_retry() {
	use std::os::unix::fs::PermissionsExt;

	let source = tempfile::tempdir().unwrap();
	let parent = tempfile::tempdir().unwrap();
	let target = parent.path().join("target");
	let private = source.path().join("documents");
	let file = private.join("notes");
	fs::create_dir(&private).unwrap();
	fs::write(&file, b"notes").unwrap();
	fs::set_permissions(source.path(), fs::Permissions::from_mode(0o750)).unwrap();
	fs::set_permissions(&private, fs::Permissions::from_mode(0o710)).unwrap();
	fs::set_permissions(&file, fs::Permissions::from_mode(0o640)).unwrap();

	let mut journal = journal(source.path(), &target);
	let copied = target.join("documents");
	let copied_file = copied.join("notes");
	let mode = |path: &Path| path.metadata().unwrap().permissions().mode() & 0o7777;
	let control = Control::default();
	let mut inspected_stage = false;
	copy_workspace(&mut journal, false, &control, |journal| {
		if journal.content[1].staged_identity.is_some()
			&& journal.content[1].destination_identity.is_none()
		{
			assert_eq!(mode(&target), 0o700);
			assert_eq!(mode(&copied), 0o700);
			let stage = super::copy::temporary(&copied_file, journal, 1);
			assert_eq!(mode(&stage), 0o600);
			inspected_stage = true;
		}
		Ok(())
	})
	.unwrap();
	assert!(inspected_stage);
	assert_eq!(mode(&target), 0o750);
	assert_eq!(mode(&copied), 0o710);
	assert_eq!(mode(&copied_file), 0o640);

	for (entry, original) in [(&copied_file, 0o640), (&copied, 0o710), (&target, 0o750)] {
		fs::set_permissions(entry, fs::Permissions::from_mode(0o755)).unwrap();
		assert!(copy_workspace(&mut journal, false, &control, |_| Ok(())).is_err());
		assert_eq!(mode(entry), 0o755);
		fs::set_permissions(entry, fs::Permissions::from_mode(original)).unwrap();
	}
}
