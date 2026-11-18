use std::fs;
use workyard_core::{Control, ProblemCode, RelocationState};
use workyard_state::{Relocation, Store, snapshot_status};

#[test]
fn nested_destination_state_shares_a_directory_with_authored_content() {
	let fixture = tempfile::tempdir().unwrap();
	let base = fixture.path().canonicalize().unwrap();
	let old = base.join("old");
	let new = base.join("new");
	let source_state = old.join(".workyard");
	let destination_state = new.join("config/state");
	drop(Store::open(&old, &source_state).unwrap());
	fs::create_dir(old.join("config")).unwrap();
	fs::write(old.join("config/notes"), "authored notes").unwrap();

	let control = Control::default();
	let mut copy = Relocation::open_copy_control(
		&source_state,
		Some(&old),
		&new,
		Some(&destination_state),
		&control,
	)
	.unwrap();
	copy.begin(Vec::new()).unwrap();
	copy.transfer(false, &control).unwrap();
	copy.activate(false).unwrap();
	copy.cleanup().unwrap();
	drop(copy);

	for root in [&old, &new] {
		assert_eq!(
			fs::read(root.join("config/notes")).unwrap(),
			b"authored notes"
		);
	}
	let snapshot = snapshot_status(&source_state).unwrap().unwrap();
	assert_eq!(snapshot.tasks_root, old);
	assert_eq!(snapshot.active_tasks_root, new);
	assert_eq!(snapshot.active_state_dir, destination_state);
	assert!(!snapshot.partial);
	assert!(Store::open_existing(&destination_state, None).is_ok());
}

#[test]
fn destination_state_cannot_cover_an_authored_directory() {
	let fixture = tempfile::tempdir().unwrap();
	let base = fixture.path().canonicalize().unwrap();
	let old = base.join("old");
	let source_state = old.join(".workyard");
	let new = base.join("new");
	drop(Store::open(&old, &source_state).unwrap());
	fs::create_dir(old.join("config")).unwrap();
	fs::write(old.join("config/notes"), "authored notes").unwrap();

	let error = Relocation::open_copy_control(
		&source_state,
		Some(&old),
		&new,
		Some(&new.join("config")),
		&Control::default(),
	)
	.err()
	.expect("overlap must be refused before mutation");

	assert!(error.message.contains("overlaps destination state"));
	assert_eq!(
		fs::read(old.join("config/notes")).unwrap(),
		b"authored notes"
	);
	assert!(!source_state.join("relocation.json").exists());
	assert!(!new.exists());
	assert!(Store::open_existing(&source_state, None).is_ok());
}

#[test]
fn forward_recovery_refuses_files_added_since_copy_verification() {
	for changed_source in [true, false] {
		let fixture = tempfile::tempdir().unwrap();
		let base = fixture.path().canonicalize().unwrap();
		let old = base.join("old");
		let new = base.join("new");
		let state = old.join(".workyard");
		drop(Store::open(&old, &state).unwrap());
		fs::create_dir(old.join("notes")).unwrap();

		let control = Control::default();
		let mut copy =
			Relocation::open_copy_control(&state, Some(&old), &new, None, &control).unwrap();
		copy.begin(Vec::new()).unwrap();
		copy.transfer(false, &control).unwrap();
		drop(copy);

		let (changed, other) = if changed_source {
			(&old, &new)
		} else {
			(&new, &old)
		};
		let added = changed.join("notes/addition");
		fs::write(&added, "new entry").unwrap();
		let mut recovered = Relocation::recover(&state).unwrap();
		let error = recovered.transfer(false, &control).unwrap_err();
		assert_eq!(error.code, ProblemCode::RepositoryChanged);
		assert_eq!(fs::read(added).unwrap(), b"new entry");
		assert!(!other.join("notes/addition").exists());
		assert!(!recovered.activated());
	}
}
