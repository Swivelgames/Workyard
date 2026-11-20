#[path = "recover_noop/scope.rs"]
mod scope;
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
	init_workspace(&root);
	create(&root, &source);
	fs::write(root.join("tasks/feat/task/api/file"), "authored changes").unwrap();
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
	let rejected = workyard(&root, &["recover", "feat/task", "--force"]);
	assert!(!rejected.status.success());
	assert_eq!(snapshot(&state), before);
}

#[test]
fn repair_then_repeat_noop_preserves_success_despite_stale_failed_effect_provenance() {
	let fixture = tempfile::tempdir().unwrap();
	let source = repo(fixture.path(), "api");
	let root = fixture.path().join("workspace");
	init_workspace(&root);
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

	let status = workyard(&root, &["status", "feat/task", "--json"]);
	assert_ok(&status);
	let status: serde_json::Value = serde_json::from_slice(&status.stdout).unwrap();
	assert_eq!(status["result"]["tasks"][0]["condition"], "healthy");
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
	init_workspace(&root);
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
	init_workspace(&root);
	create(&root, &source);
	let destination = fixture.path().join("moved");
	git(
		&source,
		&[
			"worktree",
			"move",
			text(&root.join("tasks/feat/task/api")),
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

#[test]
fn missing_worktree_failure_preserves_evidence_and_external_repair_restores_health() {
	let fixture = tempfile::tempdir().unwrap();
	let source = repo(fixture.path(), "api");
	let root = fixture.path().join("workspace");
	init_workspace(&root);
	create(&root, &source);
	let destination = fixture.path().join("external");
	fs::rename(root.join("tasks/feat/task/api"), &destination).unwrap();
	let state = root.join(".workyard");
	let before = snapshot(&state);

	let failed = workyard(&root, &["recover", "feat/task"]);
	assert!(!failed.status.success());
	assert_eq!(snapshot(&state), before);
	git(&source, &["worktree", "repair", text(&destination)]);
	let status = workyard(&root, &["status", "feat/task", "--json"]);
	assert_ok(&status);
	let status: serde_json::Value = serde_json::from_slice(&status.stdout).unwrap();
	assert_eq!(status["result"]["tasks"][0]["condition"], "healthy");
	assert_eq!(snapshot(&state), before);
}

#[test]
fn recovery_repairs_unique_manually_renamed_task_child_and_preserves_membership() {
	let fixture = tempfile::tempdir().unwrap();
	let source = repo(fixture.path(), "api");
	let root = fixture.path().join("workspace");
	init_workspace(&root);
	create(&root, &source);
	let destination = root.join("tasks/feat/task/renamed");
	fs::rename(root.join("tasks/feat/task/api"), &destination).unwrap();
	fs::write(destination.join("file"), "authored edits").unwrap();

	assert_ok(&workyard(&root, &["recover", "feat/task"]));
	let task = record(&root.join(".workyard"));
	assert_eq!(task["participants"][0]["name"], "api");
	assert_eq!(
		task["participants"][0]["destination"],
		text(destination.strip_prefix(&root).unwrap())
	);
	assert_eq!(
		fs::read_to_string(destination.join("file")).unwrap(),
		"authored edits"
	);
	assert!(git(&source, &["worktree", "list", "--porcelain"]).contains(text(&destination)));
	let before = snapshot(&root.join(".workyard"));
	assert_ok(&workyard(&root, &["recover", "feat/task"]));
	assert_eq!(snapshot(&root.join(".workyard")), before);
}

#[test]
fn recovery_finds_renamed_worktree_when_original_path_is_reused() {
	let fixture = tempfile::tempdir().unwrap();
	let source = repo(fixture.path(), "api");
	let root = fixture.path().join("workspace");
	init_workspace(&root);
	create(&root, &source);
	let original = root.join("tasks/feat/task/api");
	let renamed = root.join("tasks/feat/task/oops");
	fs::rename(&original, &renamed).unwrap();
	fs::create_dir(&original).unwrap();
	fs::write(original.join("notes"), "unrelated files").unwrap();
	fs::write(renamed.join("file"), "authored edits").unwrap();

	let repaired = workyard(&root, &["recover", "feat/task"]);
	assert_ok(&repaired);
	let progress = String::from_utf8(repaired.stderr).unwrap();
	assert!(
		progress.contains("api: repairing Git worktree link"),
		"{progress}"
	);

	let task = record(&root.join(".workyard"));
	assert_eq!(task["participants"][0]["name"], "api");
	assert_eq!(
		task["participants"][0]["destination"],
		text(renamed.strip_prefix(&root).unwrap())
	);
	assert_eq!(
		fs::read_to_string(original.join("notes")).unwrap(),
		"unrelated files"
	);
	assert!(!original.join(".git").exists());
	assert_eq!(
		fs::read_to_string(renamed.join("file")).unwrap(),
		"authored edits"
	);
	assert!(git(&source, &["worktree", "list", "--porcelain"]).contains(text(&renamed)));
}

#[cfg(unix)]
#[test]
fn recovery_accepts_a_registered_worktree_with_a_symlinked_git_pointer() {
	let fixture = tempfile::tempdir().unwrap();
	let source = repo(fixture.path(), "api");
	let root = fixture.path().join("workspace");
	init_workspace(&root);
	create(&root, &source);
	let worktree = root.join("tasks/feat/task/api");
	fs::rename(worktree.join(".git"), worktree.join("git-pointer")).unwrap();
	std::os::unix::fs::symlink("git-pointer", worktree.join(".git")).unwrap();
	let state = root.join(".workyard");
	let before = snapshot(&state);

	let result = workyard(&root, &["recover", "feat/task"]);

	assert_ok(&result);
	assert_eq!(snapshot(&state), before);
	assert!(worktree.join(".git").is_symlink());
}

#[test]
fn ambiguous_task_children_are_not_repaired_or_published() {
	let fixture = tempfile::tempdir().unwrap();
	let source = repo(fixture.path(), "api");
	let root = fixture.path().join("workspace");
	init_workspace(&root);
	create(&root, &source);
	let first = root.join("tasks/feat/task/first");
	let second = root.join("tasks/feat/task/second");
	fs::rename(root.join("tasks/feat/task/api"), &first).unwrap();
	fs::create_dir(&second).unwrap();
	fs::copy(first.join(".git"), second.join(".git")).unwrap();
	let before = snapshot(&root.join(".workyard"));
	let git_before = git(&source, &["worktree", "list", "--porcelain"]);

	let failed = workyard(&root, &["recover", "feat/task"]);
	assert!(!failed.status.success());
	assert_eq!(snapshot(&root.join(".workyard")), before);
	assert_eq!(
		git(&source, &["worktree", "list", "--porcelain"]),
		git_before
	);
	assert!(first.exists() && second.exists());
}
