mod support;
use std::{
	fs,
	path::{Path, PathBuf},
	process::Command,
};
use support::*;

fn state_file(root: &Path, directory: &str, extension: &str) -> PathBuf {
	fs::read_dir(root.join(".workyard").join(directory))
		.unwrap()
		.map(|entry| entry.unwrap().path())
		.find(|path| path.extension().is_some_and(|e| e == extension))
		.unwrap()
}

fn create(root: &Path, source: &Path, task: &str) {
	assert_ok(&workyard(
		root,
		&[
			"create",
			task,
			"--repo",
			text(source),
			"--from",
			"main",
			"--no-fetch",
		],
	));
}

#[test]
fn healthy_recovery_preserves_commits_and_reconstructs_missing_record() {
	let fixture = tempfile::tempdir().unwrap();
	let api = repo(fixture.path(), "api");
	let root = fixture.path().join("workspace");
	fs::create_dir(&root).unwrap();
	create(&root, &api, "feat/task");
	let tree = root.join("feat/task/api");
	fs::write(tree.join("authored"), "keep me").unwrap();
	git(&tree, &["add", "authored"]);
	git(&tree, &["commit", "-m", "user work"]);
	let before = git(&tree, &["rev-parse", "HEAD"]);
	let evidence = state_file(&root, "evidence", "effects");
	let bytes = fs::read(&evidence).unwrap();
	fs::remove_file(state_file(&root, "tasks", "json")).unwrap();
	for cwd in [&tree, &root] {
		assert_ok(&workyard(cwd, &["recover", "feat/task"]));
		let saved = record(&root.join(".workyard"));
		assert_eq!(saved["operation"]["kind"], "recover");
		assert_eq!(saved["operation"]["continues"], "create");
	}
	assert_eq!(git(&tree, &["rev-parse", "HEAD"]), before);
	assert_eq!(fs::read(evidence).unwrap(), bytes);
	assert_eq!(
		record(&root.join(".workyard"))["operation"]["outcome"],
		"succeeded"
	);
}

#[test]
fn missing_corrupt_and_mismatched_state_never_bootstraps_or_adopts() {
	let fixture = tempfile::tempdir().unwrap();
	let missing = fixture.path().join("absent");
	let result = workyard(
		fixture.path(),
		&["recover", "feat/task", "--state-dir", text(&missing)],
	);
	assert!(!result.status.success());
	assert!(!missing.exists());
	let api = repo(fixture.path(), "api");
	let root = fixture.path().join("workspace");
	fs::create_dir(&root).unwrap();
	create(&root, &api, "feat/task");
	let task_file = state_file(&root, "tasks", "json");
	let original = fs::read(&task_file).unwrap();
	fs::write(&task_file, "broken").unwrap();
	assert!(!workyard(&root, &["recover", "feat/task"]).status.success());
	assert_eq!(fs::read(&task_file).unwrap(), b"broken");
	fs::write(&task_file, original).unwrap();
	let evidence = state_file(&root, "evidence", "effects");
	let mut value: serde_json::Value =
		serde_json::from_slice(&fs::read(&evidence).unwrap()).unwrap();
	value["task"]["participants"][0]["name"] = "substitution".into();
	fs::write(&evidence, serde_json::to_vec(&value).unwrap()).unwrap();
	assert!(!workyard(&root, &["recover", "feat/task"]).status.success());
	assert_eq!(
		git(&api, &["worktree", "list", "--porcelain"])
			.matches("worktree ")
			.count(),
		2
	);
}

#[test]
fn failed_pull_requires_acceptance_and_preserves_next_participant() {
	let fixture = tempfile::tempdir().unwrap();
	let api = repo(fixture.path(), "api");
	let web = repo(fixture.path(), "web");
	let root = fixture.path().join("workspace");
	fs::create_dir(&root).unwrap();
	let first = workyard(
		&root,
		&[
			"create",
			"feat/task",
			"--repo",
			text(&api),
			"--repo",
			text(&web),
			"--from",
			"main",
			"--pull",
		],
	);
	assert!(!first.status.success());
	let tree = root.join("feat/task/api");
	fs::write(tree.join("authored"), "preserved").unwrap();
	assert!(!workyard(&root, &["recover", "feat/task"]).status.success());
	assert!(!root.join("feat/task/web").exists());
	let accepted = workyard(&root, &["recover", "feat/task", "--accept-pull", "api"]);
	assert!(!accepted.status.success()); // web's own pull now fails; api is settled.
	assert_eq!(
		record(&root.join(".workyard"))["participants"][0]["phase"],
		"completed"
	);
	assert!(root.join("feat/task/web/.git").exists());
	assert_ok(&workyard(
		&root,
		&["recover", "feat/task", "--accept-pull", "web"],
	));
	assert_eq!(
		fs::read_to_string(tree.join("authored")).unwrap(),
		"preserved"
	);
}

#[test]
fn acknowledgement_never_bypasses_pending_writer_or_git_operation() {
	let fixture = tempfile::tempdir().unwrap();
	let api = repo(fixture.path(), "api");
	let root = fixture.path().join("workspace");
	fs::create_dir(&root).unwrap();
	assert!(
		!workyard(
			&root,
			&[
				"create",
				"feat/task",
				"--repo",
				text(&api),
				"--from",
				"main",
				"--pull"
			]
		)
		.status
		.success()
	);
	let tree = root.join("feat/task/api");
	let git_dir = PathBuf::from(git(&tree, &["rev-parse", "--absolute-git-dir"]));
	fs::write(
		git_dir.join("MERGE_HEAD"),
		git(&tree, &["rev-parse", "HEAD"]),
	)
	.unwrap();
	assert!(
		!workyard(&root, &["recover", "feat/task", "--accept-pull", "api"])
			.status
			.success()
	);
	assert!(git_dir.join("MERGE_HEAD").exists());
	fs::remove_file(git_dir.join("MERGE_HEAD")).unwrap();
	let evidence = state_file(&root, "evidence", "effects");
	let mut value: serde_json::Value =
		serde_json::from_slice(&fs::read(&evidence).unwrap()).unwrap();
	value["pending"] = 0.into();
	fs::write(&evidence, serde_json::to_vec(&value).unwrap()).unwrap();
	let before = fs::read(&evidence).unwrap();
	let result = workyard(&root, &["recover", "feat/task", "--accept-pull", "api"]);
	assert!(!result.status.success());
	assert!(String::from_utf8_lossy(&result.stdout).contains("no durable proof"));
	assert_eq!(fs::read(&evidence).unwrap(), before);
}

#[test]
fn repaired_main_and_linked_moves_refresh_git_locations() {
	let fixture = tempfile::tempdir().unwrap();
	let api = repo(fixture.path(), "api");
	let root = fixture.path().join("workspace");
	fs::create_dir(&root).unwrap();
	create(&root, &api, "feat/task");
	let moved = fixture.path().join("moved-main");
	fs::rename(&api, &moved).unwrap();
	git(&moved, &["worktree", "repair"]);
	assert_ok(&workyard(&root, &["recover", "feat/task"]));
	let old_tree = root.join("feat/task/api");
	let moved_tree = fixture.path().join("moved-tree");
	git(
		&moved,
		&["worktree", "move", text(&old_tree), text(&moved_tree)],
	);
	assert_ok(&workyard(&root, &["recover", "feat/task"]));
	assert_eq!(
		record(&root.join(".workyard"))["participants"][0]["destination"],
		text(&moved_tree)
	);
}

#[test]
fn rpc_recovery_returns_same_completed_participants_as_cli() {
	use std::io::Write;
	let fixture = tempfile::tempdir().unwrap();
	let api = repo(fixture.path(), "api");
	let root = fixture.path().join("workspace");
	fs::create_dir(&root).unwrap();
	create(&root, &api, "feat/task");
	for _ in 0..2 {
		let mut child = Command::new(env!("CARGO_BIN_EXE_workyard"))
			.current_dir(&root)
			.arg("rpc")
			.stdin(std::process::Stdio::piped())
			.stdout(std::process::Stdio::piped())
			.spawn()
			.unwrap();
		let request = serde_json::json!({"jsonrpc":"2.0","id":1,"method":"invoke","params":{"action":["recover"],"input":{"task":"feat/task"}}});
		writeln!(child.stdin.take().unwrap(), "{request}").unwrap();
		let output = child.wait_with_output().unwrap();
		assert_ok(&output);
		let response: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
		assert_eq!(response["result"]["operation"]["outcome"], "succeeded");
		assert_eq!(response["result"]["operation"]["kind"], "recover");
		assert_eq!(response["result"]["operation"]["continues"], "create");
		assert_eq!(response["result"]["participants"][0]["phase"], "completed");
		let saved = record(&root.join(".workyard"));
		assert_eq!(saved["operation"]["kind"], "recover");
		assert_eq!(saved["operation"]["continues"], "create");
	}
}

#[cfg(unix)]
#[test]
fn real_git_success_publication_gap_recovers_remaining_work() {
	use std::os::unix::fs::PermissionsExt;
	let fixture = tempfile::tempdir().unwrap();
	let api = repo(fixture.path(), "api");
	let web = repo(fixture.path(), "web");
	let root = fixture.path().join("workspace");
	fs::create_dir(&root).unwrap();
	let tasks = root.join(".workyard/tasks");
	let hook = api.join(".git/hooks/post-checkout");
	fs::write(&hook, "#!/bin/sh\nchmod 500 \"$WORKYARD_FAULT_DIR\"\n").unwrap();
	fs::set_permissions(&hook, fs::Permissions::from_mode(0o700)).unwrap();
	let result = Command::new(env!("CARGO_BIN_EXE_workyard"))
		.current_dir(&root)
		.args([
			"create",
			"feat/task",
			"--repo",
			text(&api),
			"--repo",
			text(&web),
			"--from",
			"main",
			"--no-fetch",
		])
		.env("WORKYARD_FAULT_DIR", &tasks)
		.output()
		.unwrap();
	fs::set_permissions(&tasks, fs::Permissions::from_mode(0o700)).unwrap();
	assert!(!result.status.success());
	let tree = root.join("feat/task/api");
	fs::write(tree.join("authored"), "kept").unwrap();
	fs::remove_file(hook).unwrap();
	fs::write(web.join("file"), "new source tip").unwrap();
	git(&web, &["commit", "-am", "advance source"]);
	assert_ok(&workyard(&root, &["recover", "feat/task"]));
	assert_eq!(fs::read_to_string(tree.join("authored")).unwrap(), "kept");
	assert!(root.join("feat/task/web/.git").exists());
	assert_eq!(
		git(&root.join("feat/task/web"), &["rev-parse", "HEAD"]),
		git(&web, &["rev-parse", "main"])
	);
}

#[test]
fn healthy_remote_task_does_not_fetch_for_freshness() {
	let fixture = tempfile::tempdir().unwrap();
	let (api, remote) = remote_clone(fixture.path(), "api");
	let root = fixture.path().join("workspace");
	fs::create_dir(&root).unwrap();
	assert_ok(&workyard(
		&root,
		&[
			"create",
			"feat/task",
			"--repo",
			text(&api),
			"--from",
			"main",
		],
	));
	fs::remove_dir_all(remote).unwrap();
	assert_ok(&workyard(&root, &["recover", "feat/task"]));
}

#[test]
fn replaced_worktree_identity_is_not_adopted() {
	let fixture = tempfile::tempdir().unwrap();
	let api = repo(fixture.path(), "api");
	let root = fixture.path().join("workspace");
	fs::create_dir(&root).unwrap();
	create(&root, &api, "feat/task");
	let tree = root.join("feat/task/api");
	let original_git_dir = PathBuf::from(git(&tree, &["rev-parse", "--absolute-git-dir"]));
	// Keep the old inode allocated while a replacement registration is created.
	let held = fixture.path().join("old-administration");
	fs::rename(&original_git_dir, &held).unwrap();
	fs::remove_dir_all(&tree).unwrap();
	git(&api, &["worktree", "add", text(&tree), "feat/task"]);
	let result = workyard(&root, &["recover", "feat/task"]);
	assert!(!result.status.success());
	assert!(String::from_utf8_lossy(&result.stdout).contains("administrative identity changed"));
}

#[test]
fn explicit_commit_source_stays_exact_during_recovery() {
	let fixture = tempfile::tempdir().unwrap();
	let api = repo(fixture.path(), "api");
	let web = repo(fixture.path(), "web");
	let commit = git(&web, &["rev-parse", "HEAD"]);
	let root = fixture.path().join("workspace");
	fs::create_dir(&root).unwrap();
	assert!(
		!workyard(
			&root,
			&[
				"create",
				"feat/task",
				"--repo",
				text(&api),
				"--repo",
				text(&web),
				"--repo-from",
				&commit,
				"--from",
				"main",
				"--pull"
			]
		)
		.status
		.success()
	);
	fs::write(web.join("file"), "advanced").unwrap();
	git(&web, &["commit", "-am", "advance"]);
	assert!(
		!workyard(&root, &["recover", "feat/task", "--accept-pull", "api"])
			.status
			.success()
	);
	assert_eq!(
		git(&root.join("feat/task/web"), &["rev-parse", "HEAD"]),
		commit
	);
}

#[cfg(unix)]
#[test]
fn registered_native_path_ending_in_newline_is_preserved() {
	let fixture = tempfile::tempdir().unwrap();
	let api = repo(fixture.path(), "api");
	let root = fixture.path().join("workspace");
	fs::create_dir(&root).unwrap();
	create(&root, &api, "feat/task");
	let moved = fixture.path().join("moved\n");
	git(
		&api,
		&[
			"worktree",
			"move",
			text(&root.join("feat/task/api")),
			text(&moved),
		],
	);
	assert_ok(&workyard(&root, &["recover", "feat/task"]));
	assert_eq!(
		record(&root.join(".workyard"))["participants"][0]["destination"],
		text(&moved)
	);
}

#[test]
fn cancellation_during_recovery_preserves_durable_completed_participant() {
	use std::sync::{
		Arc,
		atomic::{AtomicBool, Ordering},
	};
	struct CancelAfterAcceptance(Arc<AtomicBool>);
	impl workyard_core::Observer for CancelAfterAcceptance {
		async fn phase(
			&self,
			_: &str,
			participant: &str,
			phase: &str,
		) -> Result<(), workyard_core::Problem> {
			if participant == "api" && phase == "completed" {
				self.0.store(true, Ordering::SeqCst);
			}
			Ok(())
		}
	}
	let fixture = tempfile::tempdir().unwrap();
	let api = repo(fixture.path(), "api");
	let web = repo(fixture.path(), "web");
	let root = fixture.path().join("workspace");
	fs::create_dir(&root).unwrap();
	assert!(
		!workyard(
			&root,
			&[
				"create",
				"feat/task",
				"--repo",
				text(&api),
				"--repo",
				text(&web),
				"--from",
				"main",
				"--pull"
			]
		)
		.status
		.success()
	);
	let cancelled = Arc::new(AtomicBool::new(false));
	let signal = cancelled.clone();
	let control = workyard_core::Control::new(move || signal.load(Ordering::SeqCst));
	let store = workyard_state::Store::open_existing(&root.join(".workyard"), None).unwrap();
	let (task, _) = smol::block_on(workyard_core::recover(
		"feat/task",
		&["api".into()],
		&workyard_git::Runner::default(),
		&control,
		&CancelAfterAcceptance(cancelled),
		&store,
	))
	.unwrap();
	assert_eq!(
		task.operation.outcome,
		workyard_core::OperationOutcome::Cancelled
	);
	assert_eq!(task.participants[0].phase, workyard_core::Phase::Completed);
	assert!(!root.join("feat/task/web").exists());
	assert_eq!(
		record(&root.join(".workyard"))["operation"]["outcome"],
		"cancelled"
	);
	drop(store);
	assert!(!workyard(&root, &["recover", "feat/task"]).status.success());
	assert_ok(&workyard(
		&root,
		&["recover", "feat/task", "--accept-pull", "web"],
	));
}

#[test]
fn failed_fetch_recovers_against_restored_advanced_remote() {
	let fixture = tempfile::tempdir().unwrap();
	let (api, remote) = remote_clone(fixture.path(), "api");
	let unavailable = fixture.path().join("remote-offline.git");
	fs::rename(&remote, &unavailable).unwrap();
	let root = fixture.path().join("workspace");
	fs::create_dir(&root).unwrap();

	let failed = workyard(
		&root,
		&[
			"create",
			"feat/task",
			"--repo",
			text(&api),
			"--from",
			"origin/main",
		],
	);
	assert!(!failed.status.success());
	assert_eq!(
		record(&root.join(".workyard"))["participants"][0]["phase"],
		"fetching"
	);
	assert!(!root.join("feat/task/api").exists());

	let upstream = fixture.path().join("api-upstream");
	fs::write(upstream.join("file"), "remote advanced during interruption").unwrap();
	git(&upstream, &["commit", "-am", "advance remote"]);
	git(&upstream, &["push", text(&unavailable), "main"]);
	let expected = git(&upstream, &["rev-parse", "HEAD"]);
	fs::rename(&unavailable, &remote).unwrap();

	assert_ok(&workyard(&root, &["recover", "feat/task"]));
	assert_eq!(
		git(&root.join("feat/task/api"), &["rev-parse", "HEAD"]),
		expected
	);
	let durable = record(&root.join(".workyard"));
	assert_eq!(durable["operation"]["outcome"], "succeeded");
	assert_eq!(durable["participants"][0]["phase"], "completed");
	let effects: serde_json::Value =
		serde_json::from_slice(&fs::read(state_file(&root, "evidence", "effects")).unwrap())
			.unwrap();
	assert!(effects["pending"].is_null());
	assert_eq!(effects["task"]["participants"][0]["phase"], "completed");
}

#[test]
fn missing_completed_participant_never_claims_current_availability() {
	let fixture = tempfile::tempdir().unwrap();
	let api = repo(fixture.path(), "infra");
	let root = fixture.path().join("workspace");
	fs::create_dir(&root).unwrap();
	create(&root, &api, "feat/task");
	fs::remove_dir_all(root.join("feat/task/infra")).unwrap();

	let output = workyard(&root, &["recover", "feat/task"]);
	assert!(!output.status.success());
	let output = String::from_utf8(output.stdout).unwrap();
	assert!(output.contains("Recovery failed"), "{output}");
	assert!(!output.contains("Recorded progress"), "{output}");
	assert!(!output.contains("available"), "{output}");
	assert!(output.contains("workyard status"), "{output}");
	assert!(output.contains("--state-dir"), "{output}");
	assert!(output.contains("feat/task"), "{output}");

	let status = workyard(&root, &["status"]);
	let status = String::from_utf8(status.stdout).unwrap();
	assert!(
		status.contains("Recorded operation: recover (failed)"),
		"{status}"
	);
	assert!(status.contains("infra:"), "{status}");
}
