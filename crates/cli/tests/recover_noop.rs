#[allow(dead_code)]
mod support;
use std::{
	collections::BTreeMap,
	fs,
	path::{Path, PathBuf},
};
use support::*;

fn snapshot(state: &Path) -> BTreeMap<PathBuf, Vec<u8>> {
	let mut files = BTreeMap::new();
	for directory in ["tasks", "evidence"] {
		for entry in fs::read_dir(state.join(directory)).unwrap() {
			let path = entry.unwrap().path();
			files.insert(path.clone(), fs::read(path).unwrap());
		}
	}
	files
}

fn create(root: &Path, source: &Path) {
	assert_ok(&workyard(
		root,
		&[
			"create",
			"feat/task",
			"--repo",
			text(source),
			"--from",
			"main",
			"--no-fetch",
		],
	));
}

#[test]
fn healthy_dirty_task_noop_preserves_history_evidence_and_rejects_invalid_flags() {
	let fixture = tempfile::tempdir().unwrap();
	let source = repo(fixture.path(), "api");
	let root = fixture.path().join("workspace");
	fs::create_dir(&root).unwrap();
	create(&root, &source);
	fs::write(root.join("feat/task/api/file"), "authored changes").unwrap();
	let state = root.join(".workyard");
	let before = snapshot(&state);

	for _ in 0..2 {
		let output = workyard(&root, &["recover", "feat/task"]);
		assert_ok(&output);
		assert_eq!(
			String::from_utf8(output.stdout).unwrap(),
			"Nothing to recover: feat/task\n"
		);
		assert_eq!(snapshot(&state), before);
	}
	let rejected = workyard(&root, &["recover", "feat/task", "--accept-pull", "api"]);
	assert!(!rejected.status.success());
	assert_eq!(snapshot(&state), before);
}

#[test]
fn repair_then_repeat_noop_preserves_success_despite_stale_failed_effect_provenance() {
	let fixture = tempfile::tempdir().unwrap();
	let source = repo(fixture.path(), "api");
	let root = fixture.path().join("workspace");
	fs::create_dir(&root).unwrap();
	create(&root, &source);
	let state = root.join(".workyard");
	let name = workyard_state::task_filename("feat/task");
	let path = state.join("tasks").join(&name);
	let effects = state.join("evidence").join(&name).with_extension("effects");
	for (path, nested) in [(&path, false), (&effects, true)] {
		let mut value: serde_json::Value =
			serde_json::from_slice(&fs::read(path).unwrap()).unwrap();
		let task = if nested {
			&mut value["task"]
		} else {
			&mut value
		};
		task["operation"]["outcome"] = "failed".into();
		task["problem"] = serde_json::json!({"code":"git_failed","message":"previous failure","remedy":"inspect"});
		fs::write(path, serde_json::to_vec(&value).unwrap()).unwrap();
	}

	let repaired = workyard(&root, &["recover", "feat/task"]);
	assert_ok(&repaired);
	assert!(String::from_utf8_lossy(&repaired.stdout).contains("Recovery succeeded"));
	let before = snapshot(&state);
	let repeated = workyard(&root, &["recover", "feat/task"]);
	assert_ok(&repeated);
	assert!(String::from_utf8_lossy(&repeated.stdout).contains("Nothing to recover"));
	assert_eq!(snapshot(&state), before);
}

#[test]
fn retired_plain_recovery_is_noop_but_undo_stays_invalid() {
	let fixture = tempfile::tempdir().unwrap();
	let source = repo(fixture.path(), "api");
	let root = fixture.path().join("workspace");
	fs::create_dir(&root).unwrap();
	create(&root, &source);
	assert_ok(&workyard(&root, &["remove", "feat/task", "--yes"]));
	let state = root.join(".workyard");
	let before = snapshot(&state);
	let output = workyard(&root, &["recover", "feat/task"]);
	assert_ok(&output);
	assert!(String::from_utf8_lossy(&output.stdout).contains("Nothing to recover"));
	assert!(
		!workyard(&root, &["recover", "feat/task", "--undo"])
			.status
			.success()
	);
	assert_eq!(snapshot(&state), before);
}

#[test]
fn repaired_git_move_is_real_metadata_work_then_becomes_noop() {
	let fixture = tempfile::tempdir().unwrap();
	let source = repo(fixture.path(), "api");
	let root = fixture.path().join("workspace");
	fs::create_dir(&root).unwrap();
	create(&root, &source);
	let destination = fixture.path().join("moved");
	git(
		&source,
		&[
			"worktree",
			"move",
			text(&root.join("feat/task/api")),
			text(&destination),
		],
	);
	let repaired = workyard(&root, &["recover", "feat/task"]);
	assert_ok(&repaired);
	assert!(String::from_utf8_lossy(&repaired.stdout).contains("Recovery succeeded"));
	let state = root.join(".workyard");
	assert_eq!(
		record(&state)["participants"][0]["destination"],
		text(&destination)
	);
	let before = snapshot(&state);
	let repeated = workyard(&root, &["recover", "feat/task"]);
	assert_ok(&repeated);
	assert!(String::from_utf8_lossy(&repeated.stdout).contains("Nothing to recover"));
	assert_eq!(snapshot(&state), before);
}
