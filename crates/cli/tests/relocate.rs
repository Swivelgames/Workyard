#[allow(dead_code)]
mod support;
use std::{fs, path::Path};
use support::*;

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
fn root_move_preserves_dirty_content_loose_files_identity_and_subsequent_mutations() {
	let fixture = tempfile::tempdir().unwrap();
	let source = repo(fixture.path(), "api");
	let root = fixture.path().join("old");
	let target = fixture.path().join("new");
	fs::create_dir(&root).unwrap();
	create(&root, &source, "feat/one");
	create(&root, &source, "feat/two");
	fs::write(root.join("feat/one/api/file"), "authored changes").unwrap();
	fs::write(root.join("notes"), "loose authored notes").unwrap();
	let identity = git(
		&root.join("feat/one/api"),
		&["rev-parse", "--absolute-git-dir"],
	);

	let moved = workyard(
		fixture.path(),
		&[
			"relocate",
			"--old",
			text(&root),
			"--new",
			text(&target),
			"--json",
		],
	);
	assert_ok(&moved);
	assert_eq!(
		fs::read_to_string(target.join("notes")).unwrap(),
		"loose authored notes"
	);
	assert_eq!(
		fs::read_to_string(target.join("feat/one/api/file")).unwrap(),
		"authored changes"
	);
	assert_eq!(
		git(
			&target.join("feat/one/api"),
			&["rev-parse", "--absolute-git-dir"]
		),
		identity
	);
	assert!(!root.join(".workyard").exists());
	assert_ok(&workyard(&target, &["status", "feat/one"]));
	create(&target, &source, "feat/three");
	assert_ok(&workyard(&target, &["remove", "feat/two", "--yes"]));
}

#[test]
fn state_only_transfer_keeps_worktrees_and_retired_history_usable() {
	let fixture = tempfile::tempdir().unwrap();
	let source = repo(fixture.path(), "api");
	let root = fixture.path().join("root");
	let target = fixture.path().join("state");
	fs::create_dir(&root).unwrap();
	create(&root, &source, "feat/one");
	assert_ok(&workyard(&root, &["remove", "feat/one", "--yes"]));
	assert_ok(&workyard(
		&root,
		&[
			"checkout",
			"feat/one",
			"--repo",
			text(&source),
			"--no-fetch",
		],
	));
	let identity = git(
		&root.join("feat/one/api"),
		&["rev-parse", "--absolute-git-dir"],
	);
	assert_ok(&workyard(
		&root,
		&[
			"relocate",
			"--old",
			text(&root),
			"--new-state-dir",
			text(&target),
		],
	));
	assert_eq!(
		git(
			&root.join("feat/one/api"),
			&["rev-parse", "--absolute-git-dir"]
		),
		identity
	);
	assert_ok(&workyard(&root, &["list", "--state-dir", text(&target)]));
	assert_ok(&workyard(
		&root,
		&["recover", "feat/one", "--state-dir", text(&target)],
	));
}

#[test]
fn external_participant_stays_put_and_incomplete_git_operation_moves_intact() {
	let fixture = tempfile::tempdir().unwrap();
	let source = repo(fixture.path(), "api");
	let root = fixture.path().join("old");
	let target = fixture.path().join("new");
	let external = fixture.path().join("external");
	fs::create_dir(&root).unwrap();
	create(&root, &source, "feat/external");
	create(&root, &source, "feat/unfinished");
	git(
		&source,
		&[
			"worktree",
			"move",
			text(&root.join("feat/external/api")),
			text(&external),
		],
	);
	let administration = git(
		&root.join("feat/unfinished/api"),
		&["rev-parse", "--absolute-git-dir"],
	);
	fs::write(
		Path::new(&administration).join("MERGE_HEAD"),
		git(&source, &["rev-parse", "HEAD"]),
	)
	.unwrap();
	let moved = workyard(
		fixture.path(),
		&[
			"relocate",
			"--old",
			text(&root),
			"--new",
			text(&target),
			"--json",
		],
	);
	assert_ok(&moved);
	let moved: serde_json::Value = serde_json::from_slice(&moved.stdout).unwrap();
	assert_eq!(moved["result"]["warnings"][0]["location"], text(&external));
	assert!(external.join("file").exists());
	assert!(Path::new(&administration).join("MERGE_HEAD").exists());
	assert!(target.join("feat/unfinished/api/file").exists());
}

#[test]
fn pending_destination_removed_then_recreated_and_repeated_undo_targets_original_source() {
	use workyard_core::RelocationState;
	let fixture = tempfile::tempdir().unwrap();
	let source = repo(fixture.path(), "api");
	let root = fixture.path().join("old");
	let target = fixture.path().join("new");
	fs::create_dir(&root).unwrap();
	create(&root, &source, "feat/one");
	fs::write(root.join("notes"), "notes").unwrap();
	let state = root.join(".workyard");
	let mut session = workyard_state::Relocation::open(&state, Some(&root), &target, None).unwrap();
	session.begin(session.participants()).unwrap();
	session
		.transfer(false, &workyard_core::Control::default())
		.unwrap();
	drop(session);
	fs::remove_dir_all(target.join(".workyard")).unwrap();
	let denied = workyard(
		fixture.path(),
		&[
			"create",
			"feat/two",
			"--repo",
			text(&source),
			"--state-dir",
			text(&state),
			"--tasks-root",
			text(&root),
			"--no-fetch",
		],
	);
	assert!(!denied.status.success());
	assert_ok(&workyard(
		fixture.path(),
		&["recover", "--state-dir", text(&state), "--undo"],
	));
	assert!(root.join("feat/one/api/file").exists());
	assert_eq!(fs::read_to_string(root.join("notes")).unwrap(), "notes");
	assert!(!target.join("feat/one/api").exists());
	assert_ok(&workyard(&root, &["status", "feat/one"]));
}

#[test]
fn cleanup_failure_preserves_edits_and_active_destination_then_recovery_only_cleans() {
	use workyard_core::RelocationState;
	let fixture = tempfile::tempdir().unwrap();
	let source = repo(fixture.path(), "api");
	let root = fixture.path().join("root");
	let target = fixture.path().join("state");
	fs::create_dir(&root).unwrap();
	create(&root, &source, "feat/one");
	let state = root.join(".workyard");
	let mut session =
		workyard_state::Relocation::open(&state, Some(&root), &root, Some(&target)).unwrap();
	session.begin(session.participants()).unwrap();
	session.activate(false).unwrap();
	let path = state
		.join("tasks")
		.join(workyard_state::task_filename("feat/one"));
	let original = fs::read(&path).unwrap();
	fs::write(&path, b"edited source metadata").unwrap();
	assert!(session.cleanup().is_err());
	assert_eq!(fs::read(&path).unwrap(), b"edited source metadata");
	drop(session);
	assert_ok(&workyard(
		fixture.path(),
		&[
			"create",
			"feat/after-activation",
			"--repo",
			text(&source),
			"--tasks-root",
			text(&root),
			"--state-dir",
			text(&target),
			"--from",
			"main",
			"--no-fetch",
		],
	));
	let undo = workyard(
		fixture.path(),
		&["recover", "--state-dir", text(&target), "--undo"],
	);
	assert!(!undo.status.success());
	fs::write(&path, original).unwrap();
	assert_ok(&workyard(
		fixture.path(),
		&["recover", "--state-dir", text(&target)],
	));
	assert!(!state.exists());
	assert_ok(&workyard(
		&root,
		&["status", "feat/one", "--state-dir", text(&target)],
	));
}

#[cfg(unix)]
#[test]
fn cross_volume_move_preserves_links_directory_modes_and_dirty_content() {
	use std::os::unix::fs::{MetadataExt, PermissionsExt};
	let fixture = tempfile::tempdir().unwrap();
	let volume = Path::new("/dev/shm");
	if !volume.is_dir()
		|| fs::metadata(volume).unwrap().dev() == fs::metadata(fixture.path()).unwrap().dev()
	{
		return;
	}
	let target_fixture = tempfile::tempdir_in(volume).unwrap();
	let source = repo(fixture.path(), "api");
	let root = fixture.path().join("old");
	let target = target_fixture.path().join("new");
	fs::create_dir(&root).unwrap();
	create(&root, &source, "feat/one");
	fs::create_dir(root.join("private")).unwrap();
	fs::set_permissions(root.join("private"), fs::Permissions::from_mode(0o700)).unwrap();
	fs::write(root.join("private/notes"), "authored content").unwrap();
	fs::hard_link(root.join("private/notes"), root.join("linked-notes")).unwrap();
	std::os::unix::fs::symlink("private/notes", root.join("notes-link")).unwrap();
	assert_ok(&workyard(
		fixture.path(),
		&["relocate", "--old", text(&root), "--new", text(&target)],
	));
	assert_eq!(
		fs::read_to_string(target.join("notes-link")).unwrap(),
		"authored content"
	);
	assert_eq!(
		fs::metadata(target.join("private"))
			.unwrap()
			.permissions()
			.mode() & 0o777,
		0o700
	);
	assert_eq!(
		fs::metadata(target.join("private/notes")).unwrap().ino(),
		fs::metadata(target.join("linked-notes")).unwrap().ino()
	);
	assert_ok(&workyard(&target, &["status", "feat/one"]));
}

#[test]
fn admission_and_unavailable_source_confirmation_preserve_original_state() {
	let fixture = tempfile::tempdir().unwrap();
	let source = repo(fixture.path(), "api");
	let root = fixture.path().join("old");
	let target = fixture.path().join("occupied");
	fs::create_dir(&root).unwrap();
	fs::create_dir(&target).unwrap();
	fs::write(target.join("authored"), "keep").unwrap();
	create(&root, &source, "feat/one");
	let state = root.join(".workyard");
	let original = record(&state);
	for arguments in [
		vec!["relocate", "--old", text(&root), "--new", text(&target)],
		vec![
			"relocate",
			"--old",
			text(&root),
			"--new",
			text(&root.join("nested")),
		],
	] {
		assert!(!workyard(&root, &arguments).status.success());
	}
	let consent = workyard(
		&root,
		&[
			"relocate",
			"--new",
			text(&fixture.path().join("new")),
			"--json",
		],
	);
	assert!(!consent.status.success());
	assert!(String::from_utf8_lossy(&consent.stdout).contains("decision_required"));
	assert_eq!(record(&state), original);
	assert!(!state.join("relocation.json").exists());
	assert_eq!(fs::read_to_string(target.join("authored")).unwrap(), "keep");
}

#[test]
fn malformed_inventory_cannot_escape_endpoints_or_delete_unrelated_content() {
	use workyard_core::RelocationState;
	let fixture = tempfile::tempdir().unwrap();
	let source = repo(fixture.path(), "api");
	let root = fixture.path().join("old");
	let target = fixture.path().join("new");
	fs::create_dir(&root).unwrap();
	create(&root, &source, "feat/one");
	let state = root.join(".workyard");
	let mut session = workyard_state::Relocation::open(&state, Some(&root), &target, None).unwrap();
	session.begin(session.participants()).unwrap();
	drop(session);
	let journal = state.join("relocation.json");
	let mut value: serde_json::Value =
		serde_json::from_slice(&fs::read(&journal).unwrap()).unwrap();
	value["files"][0]["path"] = "../../unrelated".into();
	fs::write(&journal, serde_json::to_vec(&value).unwrap()).unwrap();
	let unrelated = fixture.path().join("unrelated");
	fs::write(&unrelated, "preserve").unwrap();
	let recovery = workyard(fixture.path(), &["recover", "--state-dir", text(&state)]);
	assert!(!recovery.status.success());
	assert_eq!(fs::read_to_string(unrelated).unwrap(), "preserve");
	assert!(root.join("feat/one/api/file").exists());
}

#[test]
fn undo_activation_resumes_cleanup_with_removed_pending_destination() {
	use workyard_core::RelocationState;
	let fixture = tempfile::tempdir().unwrap();
	let source = repo(fixture.path(), "api");
	let root = fixture.path().join("root");
	let target = fixture.path().join("state");
	fs::create_dir(&root).unwrap();
	create(&root, &source, "feat/one");
	let state = root.join(".workyard");
	let mut session =
		workyard_state::Relocation::open(&state, Some(&root), &root, Some(&target)).unwrap();
	session.begin(session.participants()).unwrap();
	session.activate(true).unwrap();
	drop(session);
	fs::remove_dir_all(&target).unwrap();
	assert_ok(&workyard(
		fixture.path(),
		&["recover", "--state-dir", text(&state)],
	));
	assert!(!target.exists());
	assert_ok(&workyard(&root, &["recover"]));
	assert!(!workyard(&root, &["recover", "--undo"]).status.success());
}

#[cfg(unix)]
#[test]
fn interrupted_cross_volume_copy_keeps_source_and_resumes_owned_partial_bytes() {
	use std::os::unix::fs::MetadataExt;
	use workyard_core::RelocationState;
	let fixture = tempfile::tempdir().unwrap();
	let volume = Path::new("/dev/shm");
	if !volume.is_dir()
		|| fs::metadata(volume).unwrap().dev() == fs::metadata(fixture.path()).unwrap().dev()
	{
		return;
	}
	let target_fixture = tempfile::tempdir_in(volume).unwrap();
	let source = repo(fixture.path(), "api");
	let root = fixture.path().join("old");
	let target = target_fixture.path().join("new");
	fs::create_dir(&root).unwrap();
	create(&root, &source, "feat/one");
	let bytes = vec![42; 2 * 1024 * 1024];
	fs::write(root.join("large.bin"), &bytes).unwrap();
	let state = root.join(".workyard");
	let mut session = workyard_state::Relocation::open(&state, Some(&root), &target, None).unwrap();
	session.begin(session.participants()).unwrap();
	let watched = target.clone();
	let control = workyard_core::Control::new(move || {
		fs::read_dir(&watched)
			.unwrap()
			.filter_map(Result::ok)
			.any(|entry| {
				entry
					.file_name()
					.to_string_lossy()
					.starts_with(".workyard-transfer-")
					&& entry.metadata().unwrap().len() >= 65536
			})
	});
	assert!(session.transfer(false, &control).is_err());
	assert_eq!(fs::read(root.join("large.bin")).unwrap(), bytes);
	drop(session);
	assert_ok(&workyard(
		fixture.path(),
		&["recover", "--state-dir", text(&state)],
	));
	assert_eq!(fs::read(target.join("large.bin")).unwrap(), bytes);
	assert!(!root.join("large.bin").exists());
}

#[test]
fn destination_selected_recovery_uses_latest_source_writer_evidence_and_read_only_info() {
	use workyard_core::RelocationState;
	let fixture = tempfile::tempdir().unwrap();
	let source = repo(fixture.path(), "api");
	let root = fixture.path().join("old");
	let target = fixture.path().join("new");
	fs::create_dir(&root).unwrap();
	create(&root, &source, "feat/one");
	let state = root.join(".workyard");
	let mut session = workyard_state::Relocation::open(&state, Some(&root), &target, None).unwrap();
	session.begin(session.participants()).unwrap();
	session.writer(true).unwrap();
	drop(session);
	let original = fs::read(state.join("relocation.json")).unwrap();
	let info = workyard(
		fixture.path(),
		&["info", "--state-dir", text(&state), "--json"],
	);
	assert_ok(&info);
	let value: serde_json::Value = serde_json::from_slice(&info.stdout).unwrap();
	assert_eq!(value["result"]["relocation"]["writer_pending"], true);
	assert_eq!(fs::read(state.join("relocation.json")).unwrap(), original);
	let recovery = workyard(
		fixture.path(),
		&[
			"recover",
			"--state-dir",
			text(&target.join(".workyard")),
			"--json",
		],
	);
	assert!(!recovery.status.success());
	assert!(String::from_utf8_lossy(&recovery.stdout).contains("resource_busy"));
	assert!(root.join("feat/one/api/file").exists());
}

#[test]
fn repaired_main_clone_move_refreshes_backing_cache_before_workspace_relocation() {
	let fixture = tempfile::tempdir().unwrap();
	let source = repo(fixture.path(), "api");
	let root = fixture.path().join("old");
	let target = fixture.path().join("new");
	fs::create_dir(&root).unwrap();
	create(&root, &source, "feat/one");
	let moved_source = fixture.path().join("moved-api");
	fs::rename(&source, &moved_source).unwrap();
	git(&moved_source, &["worktree", "repair"]);
	assert_ok(&workyard(
		fixture.path(),
		&["relocate", "--old", text(&root), text(&target)],
	));
	assert_ok(&workyard(&target, &["recover", "feat/one"]));
	assert_ok(&workyard(&target, &["status", "feat/one"]));
}

#[test]
fn cli_json_and_rpc_relocation_share_workspace_contract_and_progress() {
	use std::io::Write;
	use std::process::{Command, Stdio};
	let fixture = tempfile::tempdir().unwrap();
	let source = repo(fixture.path(), "api");
	let root = fixture.path().join("old");
	let target = fixture.path().join("new");
	let final_root = fixture.path().join("final");
	fs::create_dir(&root).unwrap();
	create(&root, &source, "feat/one");
	let cli = workyard(
		fixture.path(),
		&[
			"relocate",
			"--old-tasks-root",
			text(&root),
			"--new-tasks-root",
			text(&target),
			"--move",
			"--json",
		],
	);
	assert_ok(&cli);
	let cli: serde_json::Value = serde_json::from_slice(&cli.stdout).unwrap();
	assert_eq!(cli["result"]["placement_complete"], true);
	let mut child = Command::new(env!("CARGO_BIN_EXE_workyard"))
		.current_dir(fixture.path())
		.arg("rpc")
		.stdin(Stdio::piped())
		.stdout(Stdio::piped())
		.stderr(Stdio::piped())
		.spawn()
		.unwrap();
	let request = serde_json::json!({
		"jsonrpc": "2.0",
		"id": 1,
		"method": "invoke",
		"params": {
			"action": ["relocate"],
			"input": {"old": target, "new": final_root},
			"observe": true
		}
	});
	writeln!(child.stdin.take().unwrap(), "{request}").unwrap();
	let output = child.wait_with_output().unwrap();
	assert_ok(&output);
	let messages: Vec<serde_json::Value> = String::from_utf8(output.stdout)
		.unwrap()
		.lines()
		.map(|line| serde_json::from_str(line).unwrap())
		.collect();
	assert!(
		messages
			.iter()
			.any(|message| message["method"] == "progress")
	);
	let rpc = &messages.iter().find(|message| message["id"] == 1).unwrap()["result"];
	for field in [
		"scope",
		"placement_complete",
		"cleanup_complete",
		"operation",
	] {
		assert_eq!(cli["result"][field], rpc[field]);
	}
	assert_ok(&workyard(&final_root, &["recover", "feat/one"]));
}

#[test]
fn native_gate_contention_and_same_state_aliases_have_distinct_admission() {
	let fixture = tempfile::tempdir().unwrap();
	let source = repo(fixture.path(), "api");
	let root = fixture.path().join("old");
	let target = fixture.path().join("new");
	let state = fixture.path().join("state");
	fs::create_dir(&root).unwrap();
	assert_ok(&workyard(
		&root,
		&[
			"create",
			"feat/one",
			"--repo",
			text(&source),
			"--state-dir",
			text(&state),
			"--no-fetch",
			"--from",
			"main",
		],
	));
	let gate = workyard_state::Lock::acquire_mode(
		&state.join("workspace.lock"),
		workyard_state::LockMode::Shared,
	)
	.unwrap();
	let denied = workyard(
		fixture.path(),
		&[
			"relocate",
			"--old",
			text(&root),
			"--old-state-dir",
			text(&state),
			"--new",
			text(&target),
		],
	);
	assert!(!denied.status.success());
	assert!(!state.join("relocation.json").exists());
	drop(gate);
	assert_ok(&workyard(
		fixture.path(),
		&[
			"relocate",
			"--old",
			text(&root),
			"--old-state-dir",
			text(&state),
			"--new",
			text(&target),
			"--new-state-dir",
			text(&state.join(".")),
		],
	));
	assert!(!target.join(".workyard").exists());
	assert_ok(&workyard(
		&target,
		&["status", "feat/one", "--state-dir", text(&state)],
	));
}

#[test]
fn active_destination_deletion_cannot_recopy_stale_source() {
	use workyard_core::RelocationState;
	let fixture = tempfile::tempdir().unwrap();
	let source = repo(fixture.path(), "api");
	let root = fixture.path().join("root");
	let target = fixture.path().join("state");
	fs::create_dir(&root).unwrap();
	create(&root, &source, "feat/one");
	let state = root.join(".workyard");
	let mut session =
		workyard_state::Relocation::open(&state, Some(&root), &root, Some(&target)).unwrap();
	session.begin(session.participants()).unwrap();
	session.activate(false).unwrap();
	drop(session);
	fs::remove_dir_all(&target).unwrap();
	let denied = workyard(fixture.path(), &["recover", "--state-dir", text(&state)]);
	assert!(!denied.status.success());
	assert!(!target.exists());
	assert!(state.join("workspace.json").exists());
}

#[test]
fn pending_metadata_edits_and_unrelated_active_binding_block_activation() {
	use workyard_core::RelocationState;
	let fixture = tempfile::tempdir().unwrap();
	let source = repo(fixture.path(), "api");
	let root = fixture.path().join("root");
	let target = fixture.path().join("state");
	fs::create_dir(&root).unwrap();
	create(&root, &source, "feat/one");
	let state = root.join(".workyard");
	let mut session =
		workyard_state::Relocation::open(&state, Some(&root), &root, Some(&target)).unwrap();
	session.begin(session.participants()).unwrap();
	fs::write(target.join("authored"), "preserve").unwrap();
	assert!(session.activate(false).is_err());
	drop(session);
	assert_eq!(
		fs::read_to_string(target.join("authored")).unwrap(),
		"preserve"
	);
	let mut binding: serde_json::Value =
		serde_json::from_slice(&fs::read(state.join("workspace.json")).unwrap()).unwrap();
	binding["relocation"] = "unrelated-operation".into();
	binding["authority"] = "active".into();
	binding["state_dir"] = text(&target).into();
	fs::write(
		target.join("workspace.json"),
		serde_json::to_vec(&binding).unwrap(),
	)
	.unwrap();
	assert!(
		!workyard(fixture.path(), &["recover", "--state-dir", text(&state)])
			.status
			.success()
	);
	assert_eq!(
		fs::read_to_string(target.join("authored")).unwrap(),
		"preserve"
	);
}

#[test]
fn incomplete_task_moves_without_normalizing_its_unfinished_history() {
	let fixture = tempfile::tempdir().unwrap();
	let source = repo(fixture.path(), "api");
	let root = fixture.path().join("old");
	let target = fixture.path().join("new");
	fs::create_dir(&root).unwrap();
	create(&root, &source, "feat/one");
	git(
		&source,
		&["worktree", "remove", text(&root.join("feat/one/api"))],
	);
	let state = root.join(".workyard");
	let filename = workyard_state::task_filename("feat/one");
	for (path, nested) in [
		(state.join("tasks").join(&filename), false),
		(
			state
				.join("evidence")
				.join(&filename)
				.with_extension("effects"),
			true,
		),
	] {
		let mut value: serde_json::Value =
			serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
		let task = if nested {
			&mut value["task"]
		} else {
			&mut value
		};
		task["participants"][0]["phase"] = "not_started".into();
		task["participants"][0]["worktree_identity"] = serde_json::Value::Null;
		task["operation"]["outcome"] = "failed".into();
		fs::write(path, serde_json::to_vec(&value).unwrap()).unwrap();
	}
	assert_ok(&workyard(
		fixture.path(),
		&["relocate", "--old", text(&root), "--new", text(&target)],
	));
	let saved = record(&target.join(".workyard"));
	assert_eq!(saved["participants"][0]["phase"], "not_started");
	assert_eq!(saved["operation"]["outcome"], "failed");
	let status = workyard(&target, &["status", "feat/one", "--json"]);
	assert_ok(&status);
	let value: serde_json::Value = serde_json::from_slice(&status.stdout).unwrap();
	assert_eq!(value["result"]["tasks"][0]["condition"], "needs_attention");
}

#[test]
fn empty_authored_state_directories_survive_activation() {
	let fixture = tempfile::tempdir().unwrap();
	let source = repo(fixture.path(), "api");
	let root = fixture.path().join("root");
	let target = fixture.path().join("state");
	fs::create_dir(&root).unwrap();
	create(&root, &source, "feat/one");
	fs::create_dir_all(root.join(".workyard/authored/nested-empty")).unwrap();

	assert_ok(&workyard(
		&root,
		&[
			"relocate",
			"--old",
			text(&root),
			"--new-state-dir",
			text(&target),
		],
	));
	assert!(target.join("authored/nested-empty").is_dir());
	assert!(!root.join(".workyard").exists());
}

#[test]
fn resolved_backing_clone_inside_root_is_rejected_before_intent() {
	let fixture = tempfile::tempdir().unwrap();
	let source = repo(fixture.path(), "api");
	let root = fixture.path().join("root");
	let target = fixture.path().join("new");
	fs::create_dir(&root).unwrap();
	create(&root, &source, "feat/one");
	let moved_source = root.join("backing");
	fs::rename(&source, &moved_source).unwrap();
	git(&moved_source, &["worktree", "repair"]);
	let before = record(&root.join(".workyard"));

	let result = workyard(
		&root,
		&["relocate", "--old", text(&root), "--new", text(&target)],
	);
	assert!(!result.status.success());
	assert_eq!(record(&root.join(".workyard")), before);
	assert!(!root.join(".workyard/relocation.json").exists());
	assert!(!target.exists());
	assert!(moved_source.join(".git").is_dir());
	assert!(root.join("feat/one/api/file").is_file());
}

#[test]
fn native_nested_ref_lock_blocks_relocation_before_effects() {
	let fixture = tempfile::tempdir().unwrap();
	let source = repo(fixture.path(), "api");
	let root = fixture.path().join("root");
	let target = fixture.path().join("new");
	fs::create_dir(&root).unwrap();
	create(&root, &source, "feat/one");
	let lock = source.join(".git/refs/heads/feat/one.lock");
	fs::write(&lock, "writer evidence").unwrap();
	let before = record(&root.join(".workyard"));

	let result = workyard(
		&root,
		&["relocate", "--old", text(&root), "--new", text(&target)],
	);
	assert!(!result.status.success());
	assert_eq!(record(&root.join(".workyard")), before);
	assert_eq!(fs::read_to_string(lock).unwrap(), "writer evidence");
	assert!(!root.join(".workyard/relocation.json").exists());
	assert!(!target.exists());
}

#[test]
fn relocation_from_moving_worktree_cwd_finishes_output() {
	let fixture = tempfile::tempdir().unwrap();
	let source = repo(fixture.path(), "api");
	let root = fixture.path().join("root");
	let target = fixture.path().join("new");
	fs::create_dir(&root).unwrap();
	create(&root, &source, "feat/one");

	let result = workyard(
		&root.join("feat/one/api"),
		&["relocate", "--old", text(&root), "--new", text(&target)],
	);
	assert_ok(&result);
	assert!(String::from_utf8_lossy(&result.stdout).contains("Workspace relocation completed"));
	assert!(target.join("feat/one/api/file").is_file());
}

#[cfg(unix)]
#[test]
fn partial_cross_volume_undo_preserves_edited_or_replaced_stage_and_accepts_owned_bytes() {
	use std::os::unix::fs::MetadataExt;
	use workyard_core::RelocationState;
	for change in ["edited", "replaced", "unchanged"] {
		let fixture = tempfile::tempdir().unwrap();
		let volume = Path::new("/dev/shm");
		if !volume.is_dir()
			|| fs::metadata(volume).unwrap().dev() == fs::metadata(fixture.path()).unwrap().dev()
		{
			return;
		}
		let target_fixture = tempfile::tempdir_in(volume).unwrap();
		let source = repo(fixture.path(), "api");
		let root = fixture.path().join("old");
		let target = target_fixture.path().join("new");
		fs::create_dir(&root).unwrap();
		create(&root, &source, "feat/one");
		let bytes = vec![42; 2 * 1024 * 1024];
		fs::write(root.join("large.bin"), &bytes).unwrap();
		let state = root.join(".workyard");
		let mut session =
			workyard_state::Relocation::open(&state, Some(&root), &target, None).unwrap();
		session.begin(session.participants()).unwrap();
		let watched = target.clone();
		let control = workyard_core::Control::new(move || {
			fs::read_dir(&watched)
				.unwrap()
				.filter_map(Result::ok)
				.any(|entry| {
					entry
						.file_name()
						.to_string_lossy()
						.starts_with(".workyard-transfer-")
						&& entry.metadata().unwrap().len() >= 65536
				})
		});
		assert!(session.transfer(false, &control).is_err());
		drop(session);

		let stage = fs::read_dir(&target)
			.unwrap()
			.filter_map(Result::ok)
			.find(|entry| {
				entry
					.file_name()
					.to_string_lossy()
					.starts_with(".workyard-transfer-")
			})
			.unwrap()
			.path();
		if change == "replaced" {
			fs::rename(&stage, target.join("original-stage")).unwrap();
		}
		if change != "unchanged" {
			fs::write(&stage, b"authored staged bytes").unwrap();
		}
		let staged_bytes = fs::read(&stage).unwrap();

		let result = workyard(
			fixture.path(),
			&["recover", "--state-dir", text(&state), "--undo"],
		);
		assert_eq!(fs::read(root.join("large.bin")).unwrap(), bytes);
		if change == "unchanged" {
			assert_ok(&result);
			assert!(!stage.exists());
			assert!(root.join("feat/one/api/file").is_file());
		} else {
			assert!(!result.status.success());
			assert_eq!(fs::read(stage).unwrap(), staged_bytes);
		}
	}
}
