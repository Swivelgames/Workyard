use std::fs;
use workyard_core::{Control, RelocationState};
use workyard_state::{Relocation, Store, snapshot_status};

#[test]
fn activated_copy_finishes_snapshot_without_reading_or_recopying_workspace_content() {
	let fixture = tempfile::tempdir().unwrap();
	let base = fixture.path().canonicalize().unwrap();
	let old = base.join("old");
	let new = base.join("new");
	let state = base.join("external-state");
	drop(Store::open(&old, &state).unwrap());
	let control = Control::default();
	let mut copy = Relocation::open_copy_control(&state, Some(&old), &new, None, &control).unwrap();
	let destination_state = copy.new_state().unwrap();
	assert_eq!(destination_state, new.join(".workyard"));
	copy.begin(Vec::new()).unwrap();
	copy.transfer(false, &control).unwrap();
	copy.activate(false).unwrap();
	drop(copy);

	fs::write(new.join("after-activation"), "destination edit").unwrap();
	fs::write(old.join("after-activation"), "source edit").unwrap();
	let mut recovered = Relocation::recover(&state).unwrap();
	assert!(recovered.placement_complete());
	recovered.cleanup().unwrap();
	drop(recovered);

	let snapshot = snapshot_status(&state).unwrap().unwrap();
	assert!(!snapshot.partial);
	assert_eq!(snapshot.active_tasks_root, new);
	assert_eq!(snapshot.active_state_dir, destination_state);
	assert_eq!(
		fs::read(old.join("after-activation")).unwrap(),
		b"source edit"
	);
	assert_eq!(
		fs::read(new.join("after-activation")).unwrap(),
		b"destination edit"
	);

	let uncreated = base.join("uncreated");
	assert!(Store::open(&uncreated, &state).is_err());
	assert!(!uncreated.exists());
	assert!(Store::open_existing(&state, None).is_err());
	assert!(Relocation::recover(&state).is_err());
	assert!(snapshot_status(&destination_state).unwrap().is_none());
	assert!(Store::open_existing(&destination_state, None).is_ok());
}

#[test]
fn undo_after_lost_pending_state_keeps_a_partial_copy_marker() {
	let fixture = tempfile::tempdir().unwrap();
	let base = fixture.path().canonicalize().unwrap();
	let old = base.join("old");
	let state = old.join(".workyard");
	let new = base.join("new");
	drop(Store::open(&old, &state).unwrap());
	let control = Control::default();
	let mut copy = Relocation::open_copy_control(&state, None, &new, None, &control).unwrap();
	copy.begin(Vec::new()).unwrap();
	let destination_state = copy.new_state().unwrap();
	drop(copy);
	fs::remove_dir_all(&destination_state).unwrap();
	fs::write(new.join("notes"), "partial destination edit").unwrap();

	let mut recovered = Relocation::recover(&state).unwrap();
	recovered.activate(true).unwrap();
	drop(recovered);
	let mut cleanup = Relocation::recover(&destination_state).unwrap();
	assert!(cleanup.placement_complete());
	cleanup.cleanup().unwrap();
	drop(cleanup);

	let partial = snapshot_status(&destination_state).unwrap().unwrap();
	assert!(partial.partial);
	assert_eq!(partial.active_state_dir, state);
	assert_eq!(partial.active_tasks_root, old);
	assert_eq!(
		fs::read(new.join("notes")).unwrap(),
		b"partial destination edit"
	);
	let error = Store::open_existing(&destination_state, None)
		.err()
		.unwrap();
	assert!(error.message.contains("inactive partial copy"));
	assert!(Store::open_existing(&state, None).is_ok());
}

#[cfg(unix)]
#[test]
fn copy_initializes_colocated_state_under_a_private_root() {
	use std::os::unix::fs::PermissionsExt;

	let fixture = tempfile::tempdir().unwrap();
	let base = fixture.path().canonicalize().unwrap();
	let old = base.join("original");
	let new = base.join("copied");
	let state = old.join(".workyard");
	drop(Store::open(&old, &state).unwrap());
	fs::set_permissions(&old, fs::Permissions::from_mode(0o710)).unwrap();
	fs::create_dir(&new).unwrap();
	fs::set_permissions(&new, fs::Permissions::from_mode(0o755)).unwrap();

	let control = Control::default();
	let mut copy = Relocation::open_copy_control(&state, None, &new, None, &control).unwrap();
	copy.begin(Vec::new()).unwrap();
	assert!(new.join(".workyard/relocation.json").is_file());
	assert_eq!(new.metadata().unwrap().permissions().mode() & 0o777, 0o700);

	copy.transfer(false, &control).unwrap();
	assert_eq!(new.metadata().unwrap().permissions().mode() & 0o777, 0o710);
}

#[test]
fn source_journal_preceding_the_pending_binding_can_finish_snapshot_handoff() {
	let fixture = tempfile::tempdir().unwrap();
	let base = fixture.path().canonicalize().unwrap();
	let old = base.join("old");
	let new = base.join("new");
	let state = old.join(".workyard");
	drop(Store::open(&old, &state).unwrap());
	let manifest = state.join("workspace.json");
	let before = fs::read(&manifest).unwrap();
	let control = Control::default();
	let mut copy = Relocation::open_copy_control(&state, None, &new, None, &control).unwrap();
	copy.begin(Vec::new()).unwrap();
	drop(copy);
	fs::write(&manifest, before).unwrap();
	fs::remove_file(new.join(".workyard/relocation.json")).unwrap();

	let mut resumed = Relocation::recover(&state).unwrap();
	resumed.transfer(false, &control).unwrap();
	resumed.activate(false).unwrap();
	resumed.cleanup().unwrap();
	drop(resumed);
	assert_eq!(
		snapshot_status(&state).unwrap().unwrap().active_tasks_root,
		new
	);
}
