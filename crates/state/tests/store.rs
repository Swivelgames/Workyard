mod support;
use std::{
	fs,
	sync::{
		Arc,
		atomic::{AtomicBool, Ordering},
	},
};
use workyard_core::{Control, ProblemCode};
use workyard_state::{Lock, Store, task_filename};

#[test]
fn address_uses_exact_name_full_hash_and_bounded_readable_slug() {
	assert_ne!(
		task_filename("feat/payments"),
		task_filename("feat-payments")
	);
	assert!(task_filename("feat/payments").starts_with("feat-payments-"));
	let filename = task_filename(&"X".repeat(200));
	assert_eq!(filename.len(), 48 + 1 + 64 + 5);
}

#[test]
fn shared_workspace_and_prefix_locks_allow_independent_tasks_only() {
	let fixture = tempfile::tempdir().unwrap();
	let workspace = fixture.path().join("workspace");
	let root = workyard_state::initialize_workspace(&workspace, None).unwrap();
	let state = workspace.join(".workyard");
	let first = Store::open_existing(&state, Some(&root)).unwrap();
	let second = Store::open_existing(&state, Some(&root)).unwrap();
	let _one = first.admit("feat/one").unwrap();
	let _two = second.admit("feat/two").unwrap();
	assert_eq!(
		second.admit("feat/one").err().unwrap().code,
		ProblemCode::ResourceBusy
	);
	assert_eq!(
		second.admit("feat").err().unwrap().code,
		ProblemCode::ResourceBusy
	);
	assert_eq!(
		Lock::acquire(&state.join("workspace.lock"), false)
			.err()
			.unwrap()
			.code,
		ProblemCode::ResourceBusy
	);
}

#[test]
fn waiting_lock_is_cancellable_and_lockfile_survives_release() {
	let fixture = tempfile::tempdir().unwrap();
	let path = fixture.path().join("lock");
	let held = Lock::acquire(&path, false).unwrap();
	let cancelled = Arc::new(AtomicBool::new(false));
	let observer = cancelled.clone();
	let control = Control::new(move || observer.load(Ordering::SeqCst));
	let result = smol::block_on(async {
		let waiter = Lock::wait(&path, &control);
		let cancel = async {
			smol::Timer::after(std::time::Duration::from_millis(30)).await;
			cancelled.store(true, Ordering::SeqCst);
		};
		smol::future::zip(waiter, cancel).await.0
	});
	assert_eq!(result.err().unwrap().code, ProblemCode::Cancelled);
	drop(held);
	assert!(path.exists());
	Lock::acquire(&path, false).unwrap();
}

#[test]
fn corrupt_and_newer_state_are_never_reset() {
	let fixture = tempfile::tempdir().unwrap();
	let workspace = fixture.path().join("workspace");
	let root = workyard_state::initialize_workspace(&workspace, None).unwrap();
	let state = workspace.join(".workyard");
	drop(Store::open_existing(&state, Some(&root)).unwrap());
	for content in ["{bad", "{\"version\":999}"] {
		fs::write(state.join("workspace.json"), content).unwrap();
		assert!(Store::open_existing(&state, Some(&root)).is_err());
		assert_eq!(
			fs::read_to_string(state.join("workspace.json")).unwrap(),
			content
		);
	}
}

#[cfg(unix)]
#[test]
fn replacing_named_gate_invalidates_old_admission() {
	let fixture = tempfile::tempdir().unwrap();
	let workspace = fixture.path().join("workspace");
	let root = workyard_state::initialize_workspace(&workspace, None).unwrap();
	let state = workspace.join(".workyard");
	let store = Store::open_existing(&state, Some(&root)).unwrap();
	fs::rename(state.join("workspace.lock"), state.join("old.lock")).unwrap();
	fs::write(state.join("workspace.lock"), b"").unwrap();
	assert_eq!(
		store.verify().err().unwrap().code,
		ProblemCode::StaleIdentity
	);
}

#[test]
fn independent_intent_survives_failed_task_publication_and_blocks_recreation() {
	let workspace = support::Workspace::new();
	let root = &workspace.root;
	let state = &workspace.state;
	let store = &workspace.store;
	let guard = store.admit("feat/test").unwrap();
	fs::create_dir(&guard.record_path).unwrap();
	let mut task = support::task(root, state, "feat/test");
	task.participants.push(workyard_core::Participant {
		creation: None,
		name: "api".into(),
		source: root.join("source"),
		common_dir: root.join("source/.git"),
		backing_identity: "fixture-backing".into(),
		registration_roots: Vec::new(),
		worktree_identity: None,
		destination: root.join("feat/test/api"),
		remote: None,
		from: None,
		phase: workyard_core::Phase::NotStarted,
	});
	let error = store.begin(&guard, &task).unwrap_err();
	assert_eq!(error.code, ProblemCode::PublicationUncertain);
	assert!(guard.evidence_path.exists());
	fs::remove_dir(&guard.record_path).unwrap();
	drop(guard);
	let guard = store.admit("feat/test").unwrap();
	assert_eq!(
		store.ensure_new(&guard, "feat/test").unwrap_err().code,
		ProblemCode::TaskExists
	);
}

#[test]
fn lock_child() {
	if !std::path::Path::new("child-fixture").exists() {
		return;
	}
	let _lock = Lock::acquire(std::path::Path::new("resource.lock"), false).unwrap();
	fs::write("ready", b"ready").unwrap();
	while !std::path::Path::new("release").exists() {
		std::thread::sleep(std::time::Duration::from_millis(5));
	}
}

#[test]
fn independent_processes_observe_the_same_os_lock() {
	let fixture = tempfile::tempdir().unwrap();
	fs::write(fixture.path().join("child-fixture"), b"").unwrap();
	let mut child = std::process::Command::new(std::env::current_exe().unwrap())
		.current_dir(fixture.path())
		.args(["--exact", "lock_child", "--nocapture"])
		.stdout(std::process::Stdio::null())
		.spawn()
		.unwrap();
	while !fixture.path().join("ready").exists() {
		std::thread::sleep(std::time::Duration::from_millis(5));
	}
	let observed = Lock::acquire(&fixture.path().join("resource.lock"), false)
		.err()
		.unwrap()
		.code;
	fs::write(fixture.path().join("release"), b"").unwrap();
	assert!(child.wait().unwrap().success());
	assert_eq!(observed, ProblemCode::ResourceBusy);
	assert!(Lock::acquire(&fixture.path().join("resource.lock"), false).is_ok());
}

#[test]
fn case_aliases_and_unicode_resource_names_cannot_bypass_prefix_exclusion() {
	let fixture = tempfile::tempdir().unwrap();
	let workspace = fixture.path().join("workspace");
	let root = workyard_state::initialize_workspace(&workspace, None).unwrap();
	let state = workspace.join(".workyard");
	let store = Store::open_existing(&state, Some(&root)).unwrap();
	let guard = store.admit("Feat/Payments").unwrap();
	assert_eq!(
		store.admit("feat/payments/child").err().unwrap().code,
		ProblemCode::ResourceBusy
	);
	assert_eq!(
		store.admit("fëat/other").err().unwrap().code,
		ProblemCode::ResourceBusy
	);
	drop(guard);
	let unicode = store.admit("fëat/other").unwrap();
	assert_eq!(
		store.admit("another/task").err().unwrap().code,
		ProblemCode::ResourceBusy
	);
	drop(unicode);
	assert_ne!(
		task_filename("Feat/Payments"),
		task_filename("feat/payments")
	);
}

#[test]
fn named_lock_modes_interoperate_with_legacy_bool_calls() {
	use workyard_state::LockMode;
	let fixture = tempfile::tempdir().unwrap();
	let path = fixture.path().join("resource.lock");
	let named = Lock::acquire_mode(&path, LockMode::Shared).unwrap();
	let legacy = Lock::acquire(&path, true).unwrap();
	assert_eq!(
		Lock::acquire_mode(&path, LockMode::Exclusive)
			.err()
			.unwrap()
			.code,
		ProblemCode::ResourceBusy
	);
	drop(named);
	drop(legacy);
	let exclusive = Lock::acquire_mode(&path, LockMode::Exclusive).unwrap();
	assert_eq!(
		Lock::acquire(&path, true).err().unwrap().code,
		ProblemCode::ResourceBusy
	);
	drop(exclusive);
	assert!(Lock::acquire(&path, false).is_ok());
}
