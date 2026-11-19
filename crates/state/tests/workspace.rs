use std::{fs, path::Path, sync::Barrier, thread};
use workyard_core::ProblemCode;
use workyard_state::{Reader, initialize_workspace};

#[test]
fn record_preserves_the_configured_task_path_and_default_project_registration() {
	let fixture = tempfile::tempdir().unwrap();
	let workspace = fixture.path().canonicalize().unwrap();
	let tasks = Path::new("nested/worktrees");
	let root = initialize_workspace(&workspace, Some(tasks)).unwrap();

	let state = workspace.join(".workyard");
	let manifest: serde_json::Value =
		serde_json::from_slice(&fs::read(state.join("workspace.json")).unwrap()).unwrap();
	assert_eq!(manifest["workspace_root"], serde_json::json!(workspace));
	assert_eq!(Path::new(manifest["tasks_dir"].as_str().unwrap()), tasks);
	assert_eq!(manifest["project_roots"], serde_json::json!(["projects"]));
	assert_eq!(Reader::open(&state, None).unwrap().root(), root);
}

#[test]
fn competing_initializers_cannot_publish_a_different_layout_or_create_its_tasks() {
	let fixture = tempfile::tempdir().unwrap();
	let workspace = fixture.path();
	let barrier = Barrier::new(2);
	let attempt = |name: &str| {
		barrier.wait();
		initialize_workspace(workspace, Some(Path::new(name)))
	};

	let (a, b) = thread::scope(|scope| {
		let left = scope.spawn(|| attempt("a"));
		let right = scope.spawn(|| attempt("b"));
		(left.join().unwrap(), right.join().unwrap())
	});
	let (root, conflict, loser) = match (a, b) {
		(Ok(a), Err(b)) => (a, b, "b"),
		(Err(a), Ok(b)) => (b, a, "a"),
		result => panic!("Expected one accepted configuration: {result:?}"),
	};
	assert_eq!(conflict.code, ProblemCode::WorkspaceBindingConflict);
	assert_eq!(initialize_workspace(workspace, None).unwrap(), root);
	assert!(!workspace.join(loser).exists());
}

#[test]
fn invalid_task_directory_does_not_publish_a_marker() {
	let fixture = tempfile::tempdir().unwrap();
	let workspace = fixture.path();
	fs::write(workspace.join("file"), b"keep me").unwrap();
	for invalid in [
		"file",
		"file/nested",
		".",
		"../outside",
		".workyard/nested",
		"local/.workyard/tasks",
	] {
		assert!(initialize_workspace(workspace, Some(Path::new(invalid))).is_err());
		assert!(!workspace.join(".workyard").exists());
	}
	assert!(initialize_workspace(workspace, Some(workspace)).is_err());
	assert!(!workspace.join(".workyard").exists());
	assert_eq!(fs::read(workspace.join("file")).unwrap(), b"keep me");
}

#[cfg(unix)]
#[test]
fn an_escaping_task_symlink_is_rejected_for_default_and_explicit_layouts() {
	use std::os::unix::fs::symlink;
	let fixture = tempfile::tempdir().unwrap();
	let workspace = fixture.path().join("workspace");
	let elsewhere = fixture.path().join("elsewhere");
	fs::create_dir(&workspace).unwrap();
	fs::create_dir(&elsewhere).unwrap();
	symlink(&elsewhere, workspace.join("tasks")).unwrap();

	for tasks in [None, Some(Path::new("tasks/worktrees"))] {
		assert!(initialize_workspace(&workspace, tasks).is_err());
		assert!(!workspace.join(".workyard").exists());
	}
	assert_eq!(fs::read_dir(&elsewhere).unwrap().count(), 0);
}

#[cfg(unix)]
#[test]
fn nested_marker_symlinks_cannot_redirect_discovery_to_a_different_workspace() {
	use std::os::unix::fs::symlink;
	let outer = tempfile::tempdir().unwrap();
	initialize_workspace(outer.path(), None).unwrap();
	let nested = outer.path().join("nested");
	let bad = nested.join(".workyard");
	fs::create_dir(&nested).unwrap();
	for target in [outer.path().join(".workyard"), outer.path().join("absent")] {
		symlink(target, &bad).unwrap();
		let failure = workyard_state::find_state(&nested, None).unwrap_err();
		assert_eq!(failure.code, ProblemCode::DamagedState);
		assert!(initialize_workspace(&nested, None).is_err());
		fs::remove_file(&bad).unwrap();
	}
}

#[cfg(unix)]
#[test]
fn non_unicode_task_directory_round_trips_through_init_and_observation() {
	use std::os::unix::ffi::OsStrExt;
	let fixture = tempfile::tempdir().unwrap();
	let workspace = fixture.path().canonicalize().unwrap();
	let tasks = Path::new(std::ffi::OsStr::from_bytes(b"tasks-\xff"));
	let expected = workspace.join(tasks);
	assert_eq!(
		initialize_workspace(&workspace, Some(tasks)).unwrap(),
		expected
	);
	assert_eq!(
		Reader::open(&workspace.join(".workyard"), None)
			.unwrap()
			.root(),
		expected
	);
}

#[test]
fn malformed_markers_are_preserved_on_reinitialization() {
	let fixture = tempfile::tempdir().unwrap();
	let workspace = fixture.path();
	let state = workspace.join(".workyard");
	fs::create_dir(&state).unwrap();
	assert!(initialize_workspace(workspace, None).is_err());
	assert_eq!(fs::read_dir(&state).unwrap().count(), 0);
	assert!(!workspace.join("tasks").exists());
}

#[test]
fn a_tasks_directory_cannot_cross_a_nested_workspace() {
	let fixture = tempfile::tempdir().unwrap();
	let nested = fixture.path().join("inner");
	initialize_workspace(&nested, None).unwrap();

	let conflict =
		initialize_workspace(fixture.path(), Some(Path::new("inner/worktrees"))).unwrap_err();
	assert_eq!(conflict.code, ProblemCode::PathOverlap);
	assert!(!fixture.path().join(".workyard").exists());
	assert!(!nested.join("worktrees").exists());
}

#[cfg(unix)]
#[test]
fn a_failed_tasks_directory_creation_can_be_retried_with_a_different_path() {
	use std::os::unix::fs::PermissionsExt;

	let fixture = tempfile::tempdir().unwrap();
	let workspace = fixture.path();
	let restricted = workspace.join("restricted");
	fs::create_dir(&restricted).unwrap();
	let permissions = fs::metadata(&restricted).unwrap().permissions();
	fs::set_permissions(&restricted, fs::Permissions::from_mode(0o555)).unwrap();
	let result = initialize_workspace(workspace, Some(Path::new("restricted/tasks")));
	fs::set_permissions(&restricted, permissions).unwrap();

	// A privileged user may still create directories in a read-only parent.
	if result.is_ok() {
		assert!(restricted.join("tasks").is_dir());
		return;
	}
	let state = workspace.join(".workyard");
	assert!(!state.join("workspace.json").exists());
	let failure = Reader::open(&state, None)
		.err()
		.expect("incomplete workspace");
	assert!(failure.message.contains("initialization is incomplete"));

	let tasks = initialize_workspace(workspace, Some(Path::new("worktrees"))).unwrap();
	assert!(tasks.is_dir());
	assert_eq!(Reader::open(&state, None).unwrap().root(), tasks);
	assert!(!state.join("initializing.json").exists());
}
