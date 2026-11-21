#[allow(dead_code)]
mod support;
use serde_json::{Value, json};
use std::{
	collections::BTreeMap,
	fs,
	io::Write,
	path::{Path, PathBuf},
	process::{Command, Stdio},
};
use support::*;

fn rpc(root: &Path, action: &str, input: Value) -> Value {
	let mut child = Command::new(env!("CARGO_BIN_EXE_workyard"))
		.current_dir(root)
		.arg("rpc")
		.stdin(Stdio::piped())
		.stdout(Stdio::piped())
		.spawn()
		.unwrap();
	writeln!(
		child.stdin.take().unwrap(),
		"{}",
		json!({"jsonrpc":"2.0","id":1,"method":"invoke","params":{"action":[action],"input":input}})
	)
	.unwrap();
	let output = child.wait_with_output().unwrap();
	assert_ok(&output);
	serde_json::from_slice(&output.stdout).unwrap()
}

fn snapshot(root: &Path) -> BTreeMap<PathBuf, Vec<u8>> {
	fn visit(root: &Path, path: &Path, files: &mut BTreeMap<PathBuf, Vec<u8>>) {
		for entry in fs::read_dir(path).unwrap() {
			let path = entry.unwrap().path();
			if path.is_dir() {
				files.insert(path.strip_prefix(root).unwrap().to_path_buf(), vec![]);
				visit(root, &path, files);
			} else {
				files.insert(
					path.strip_prefix(root).unwrap().to_path_buf(),
					fs::read(path).unwrap(),
				);
			}
		}
	}
	let mut files = BTreeMap::new();
	visit(root, root, &mut files);
	files
}

fn create(root: &Path, source: &Path, task: &str) {
	assert_ok(&workyard(
		root,
		&[
			"create",
			"--no-fetch",
			"--from",
			"main",
			"--repo",
			text(source),
			task,
		],
	));
}

#[test]
fn absent_state_is_empty_with_guidance_and_never_initialized() {
	let fixture = tempfile::tempdir().unwrap();
	for action in ["list", "status"] {
		assert_ok(&workyard(fixture.path(), &[action]));
		let result = rpc(fixture.path(), action, json!({"workspace":"absent"}));
		assert_eq!(result["result"]["tasks"], json!([]));
		assert!(
			result["result"]["guidance"]
				.as_str()
				.unwrap()
				.contains("workyard init")
		);
		assert_eq!(fs::read_dir(fixture.path()).unwrap().count(), 0);
	}
}

#[test]
fn list_status_report_saved_current_and_evidence_without_writes() {
	let fixture = tempfile::tempdir().unwrap();
	let source = repo(fixture.path(), "api");
	let root = fixture.path().join("workspace");
	init_workspace(&root);
	create(&root, &source, "feat/z");
	create(&root, &source, "feat/a");
	fs::write(root.join("tasks/feat/a/api/file"), "edited").unwrap();
	let before = snapshot(fixture.path());
	let listed = rpc(&root, "list", json!({}));
	assert_eq!(listed["result"]["tasks"][0]["name"], "feat/a");
	assert_eq!(listed["result"]["tasks"][1]["name"], "feat/z");
	let status = rpc(&root, "status", json!({}));
	assert_eq!(status["result"]["tasks"].as_array().unwrap().len(), 2);
	assert_eq!(status["result"]["tasks"][0]["current"][0]["dirty"], true);
	assert_eq!(status["result"]["tasks"][1]["current"][0]["dirty"], false);
	assert_eq!(
		status["result"]["tasks"][0]["recorded"]["operation"]["outcome"],
		"succeeded"
	);
	assert_eq!(
		status["result"]["tasks"][0]["evidence"]["operation"]["outcome"],
		"pending"
	);
	let selected = rpc(&root, "status", json!({"task":"feat/a"}));
	assert_eq!(selected["result"]["tasks"].as_array().unwrap().len(), 1);
	let cli = workyard(&root, &["status", "feat/a"]);
	assert_ok(&cli);
	assert!(String::from_utf8(cli.stdout).unwrap().contains("modified"));
	assert_eq!(before, snapshot(fixture.path()));
}

#[test]
fn record_damage_and_pending_writers_do_not_hide_healthy_tasks() {
	let fixture = tempfile::tempdir().unwrap();
	let source = repo(fixture.path(), "api");
	let root = fixture.path().join("workspace");
	init_workspace(&root);
	create(&root, &source, "feat/good");
	create(&root, &source, "feat/bad");
	let state = root.join(".workyard");
	let name = workyard_state::task_filename("feat/bad");
	fs::write(state.join("tasks").join(&name), b"bad json").unwrap();
	let effect_path = state.join("evidence").join(&name).with_extension("effects");
	let mut effect: Value = serde_json::from_slice(&fs::read(&effect_path).unwrap()).unwrap();
	effect["pending"] = json!(0);
	fs::write(&effect_path, serde_json::to_vec_pretty(&effect).unwrap()).unwrap();
	let lock_path = state
		.join("locks")
		.join(workyard_state::task_filename("feat/bad"))
		.with_extension("lock");
	let before = snapshot(fixture.path());
	let writer = workyard_state::Lock::acquire(&lock_path, false).unwrap();
	let result = rpc(&root, "status", json!({}));
	let tasks = result["result"]["tasks"].as_array().unwrap();
	assert_eq!(tasks.len(), 2);
	assert_eq!(tasks[0]["name"], "feat/bad");
	assert_eq!(tasks[0]["busy"], true);
	assert_eq!(tasks[0]["pending_participant"], 0);
	let cli = workyard(&root, &["status", "feat/bad"]);
	assert_ok(&cli);
	assert!(
		String::from_utf8(cli.stdout)
			.unwrap()
			.contains("api: a step has an unconfirmed outcome")
	);
	assert!(!tasks[0]["problems"].as_array().unwrap().is_empty());
	assert_eq!(tasks[1]["name"], "feat/good");
	assert_eq!(tasks[1]["recorded"]["operation"]["outcome"], "succeeded");
	drop(writer);
	assert_eq!(before, snapshot(fixture.path()));
}

#[test]
fn explicit_workspace_selects_tasks_and_reports_unknown_task_without_writes() {
	let fixture = tempfile::tempdir().unwrap();
	let source = repo(fixture.path(), "api");
	let workspace = fixture.path().join("workspace");
	init_workspace(&workspace);
	create(&workspace, &source, "feat/task");

	let before = snapshot(fixture.path());
	let status = rpc(fixture.path(), "status", json!({"workspace":workspace}));
	assert_eq!(
		status["result"]["tasks_root"],
		json!(workspace.join("tasks").canonicalize().unwrap())
	);

	let unknown = rpc(
		fixture.path(),
		"status",
		json!({"workspace":workspace, "task":"feat/unknown"}),
	);
	assert_eq!(
		unknown["error"]["data"]["problem"]["code"], "task_unknown",
		"{unknown}"
	);
	assert_eq!(before, snapshot(fixture.path()));
}

#[test]
fn repaired_moves_and_renamed_branch_report_actual_identity_location() {
	let fixture = tempfile::tempdir().unwrap();
	let source = repo(fixture.path(), "api");
	let root = fixture.path().join("workspace");
	init_workspace(&root);
	create(&root, &source, "feat/task");
	let moved = fixture.path().join("moved");
	git(
		&source,
		&[
			"worktree",
			"move",
			text(&root.join("tasks/feat/task/api")),
			text(&moved),
		],
	);
	git(&moved, &["branch", "-m", "feat/renamed"]);
	let before = snapshot(fixture.path());
	let result = rpc(&root, "status", json!({"task":"feat/task"}));
	assert_eq!(
		Path::new(
			result["result"]["tasks"][0]["current"][0]["current_path"]
				.as_str()
				.unwrap()
		),
		moved
	);
	assert_eq!(
		result["result"]["tasks"][0]["current"][0]["branch"],
		"feat/renamed"
	);
	assert_eq!(before, snapshot(fixture.path()));
}

#[test]
fn malformed_diagnostics_missing_records_and_unrelated_registrations_remain_observable() {
	let fixture = tempfile::tempdir().unwrap();
	let source = repo(fixture.path(), "api");
	let root = fixture.path().join("workspace");
	init_workspace(&root);
	create(&root, &source, "feat/good");
	create(&root, &source, "feat/bad");
	let state = root.join(".workyard");
	let path = state
		.join("tasks")
		.join(workyard_state::task_filename("feat/bad"));
	let mut record: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
	record["problem"] = json!({"code":"unknown_future_code","message":"test","remedy":"test"});
	fs::write(&path, serde_json::to_vec(&record).unwrap()).unwrap();
	let unrelated = fixture.path().join("unrelated");
	git(
		&source,
		&["worktree", "add", "-b", "other", text(&unrelated)],
	);
	fs::remove_dir_all(&unrelated).unwrap();
	let before = snapshot(fixture.path());
	let result = rpc(&root, "status", json!({}));
	let tasks = result["result"]["tasks"].as_array().unwrap();
	assert_eq!(tasks.len(), 2);
	assert!(tasks[0]["problems"].as_array().unwrap().iter().any(|p| {
		p["message"]
			.as_str()
			.unwrap()
			.contains("Unknown stored problem code")
	}));
	assert_eq!(tasks[0]["recorded"]["participants"][0]["name"], "api");
	assert_eq!(
		tasks[0]["recorded"]["participants"][0]["phase"],
		"completed"
	);
	assert_eq!(tasks[0]["current"][0]["branch"], "feat/bad");
	assert_eq!(tasks[1]["current"][0]["branch"], "feat/good");
	assert_eq!(before, snapshot(fixture.path()));

	fs::remove_file(&path).unwrap();
	let effects = state
		.join("evidence")
		.join(workyard_state::task_filename("feat/bad"))
		.with_extension("effects");
	fs::write(&effects, "malformed effects").unwrap();
	let before = snapshot(fixture.path());
	let result = rpc(&root, "status", json!({"task":"feat/bad"}));
	assert!(result["result"]["tasks"][0]["recorded"].is_null());
	assert!(!result["result"]["tasks"][0]["evidence"].is_null());
	assert!(
		result["result"]["tasks"][0]["problems"]
			.as_array()
			.unwrap()
			.iter()
			.any(|problem| problem["message"]
				.as_str()
				.unwrap()
				.contains("Invalid JSON"))
	);
	assert_eq!(before, snapshot(fixture.path()));
}

#[test]
fn main_clone_repair_conflicts_and_missing_worktrees_are_reported_without_effects() {
	let fixture = tempfile::tempdir().unwrap();
	let source = repo(fixture.path(), "api");
	let root = fixture.path().join("workspace");
	init_workspace(&root);
	create(&root, &source, "feat/task");
	let tree = root.join("tasks/feat/task/api");
	let moved_source = fixture.path().join("moved-api");
	fs::rename(&source, &moved_source).unwrap();
	git(&moved_source, &["worktree", "repair"]);
	let index = moved_source.join(".git/worktrees/api/index");
	let index_before = fs::read(&index).unwrap();
	let before = snapshot(fixture.path());
	let result = rpc(&root, "status", json!({}));
	assert_eq!(
		result["result"]["tasks"][0]["current"][0]["branch"],
		"feat/task"
	);
	assert_eq!(before, snapshot(fixture.path()));
	assert_eq!(index_before, fs::read(index).unwrap());

	let git_dir = git(&tree, &["rev-parse", "--path-format=absolute", "--git-dir"]);
	fs::write(Path::new(&git_dir).join("MERGE_HEAD"), "unfinished\n").unwrap();
	let before = snapshot(fixture.path());
	let result = rpc(&root, "status", json!({}));
	assert_eq!(
		result["result"]["tasks"][0]["current"][0]["operation_in_progress"],
		true
	);
	assert_eq!(result["result"]["tasks"][0]["condition"], "unknown");
	let output = workyard(&root, &["status"]);
	assert!(
		!String::from_utf8(output.stdout)
			.unwrap()
			.contains("healthy")
	);
	assert_eq!(before, snapshot(fixture.path()));
	fs::remove_dir_all(&tree).unwrap();
	let before = snapshot(fixture.path());
	let result = rpc(&root, "status", json!({}));
	assert_eq!(
		result["result"]["tasks"][0]["current"][0]["problem"]["code"],
		"repository_changed"
	);
	assert!(result["result"]["tasks"][0]["current"][0]["dirty"].is_null());
	assert_eq!(before, snapshot(fixture.path()));
}

#[test]
fn explicit_workspace_precedes_ancestor_state_and_invalid_existing_state_is_not_empty() {
	let fixture = tempfile::tempdir().unwrap();
	let source = repo(fixture.path(), "api");
	let root = fixture.path().join("workspace");
	init_workspace(&root);
	create(&root, &source, "feat/task");
	let nested = root.join("nested");
	fs::create_dir(&nested).unwrap();
	let before = snapshot(fixture.path());
	assert_eq!(
		rpc(&nested, "list", json!({}))["result"]["tasks"][0]["name"],
		"feat/task"
	);
	assert_eq!(
		rpc(&nested, "list", json!({"workspace":"missing"}))["result"]["tasks"],
		json!([])
	);
	assert_eq!(before, snapshot(fixture.path()));
	fs::write(root.join(".workyard/workspace.json"), "bad").unwrap();
	let before = snapshot(fixture.path());
	let result = rpc(&root, "list", json!({}));
	assert_eq!(result["error"]["data"]["problem"]["code"], "damaged_state");
	assert_eq!(before, snapshot(fixture.path()));
}

#[test]
fn nul_paths_in_text_and_native_encoding_are_task_damage_without_panics() {
	let fixture = tempfile::tempdir().unwrap();
	let source = repo(fixture.path(), "api");
	let root = fixture.path().join("workspace");
	init_workspace(&root);
	create(&root, &source, "feat/good");
	create(&root, &source, "feat/bad");
	let state = root.join(".workyard");
	let path = state
		.join("tasks")
		.join(workyard_state::task_filename("feat/bad"));
	let mut record: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
	let bad_path = format!("{}\0suffix", root.display());
	let text_path = json!(bad_path);
	#[cfg(unix)]
	let encoded_path = json!({"unix": bad_path.into_bytes()});
	#[cfg(windows)]
	let encoded_path = json!({"windows": bad_path.encode_utf16().collect::<Vec<_>>()});

	for bad_path in [text_path, encoded_path] {
		record["participants"][0]["destination"] = bad_path;
		fs::write(&path, serde_json::to_vec(&record).unwrap()).unwrap();
		let before = snapshot(fixture.path());
		let output = workyard(&root, &["status"]);
		assert_ok(&output);
		let result = rpc(&root, "status", json!({}));
		let tasks = result["result"]["tasks"].as_array().unwrap();
		assert_eq!(tasks.len(), 2);
		assert!(
			tasks[0]["problems"]
				.as_array()
				.unwrap()
				.iter()
				.any(|problem| problem["message"]
					.as_str()
					.unwrap()
					.contains("unrepresentable native path"))
		);
		assert_eq!(tasks[1]["name"], "feat/good");
		assert_eq!(before, snapshot(fixture.path()));
	}
}

#[test]
fn actual_unmerged_index_is_conflicted_in_cli_and_rpc_without_writes() {
	let fixture = tempfile::tempdir().unwrap();
	let source = repo(fixture.path(), "api");
	let root = fixture.path().join("workspace");
	init_workspace(&root);
	create(&root, &source, "feat/conflict");
	let tree = root.join("tasks/feat/conflict/api");
	fs::write(tree.join("file"), "task change\n").unwrap();
	git(&tree, &["commit", "-am", "task change"]);
	fs::write(source.join("file"), "main change\n").unwrap();
	git(&source, &["commit", "-am", "main change"]);
	let merge = Command::new("git")
		.current_dir(&tree)
		.args(["merge", "main"])
		.output()
		.unwrap();
	assert!(!merge.status.success());
	assert!(!git(&tree, &["ls-files", "--unmerged"]).is_empty());

	let before = snapshot(fixture.path());
	let result = rpc(&root, "status", json!({"task":"feat/conflict"}));
	assert_eq!(
		result["result"]["tasks"][0]["current"][0]["conflicted"],
		true
	);
	let cli = workyard(&root, &["status", "feat/conflict"]);
	assert_ok(&cli);
	assert!(
		String::from_utf8(cli.stdout)
			.unwrap()
			.contains("conflicted")
	);
	assert_eq!(before, snapshot(fixture.path()));
}

fn legacy_task(value: &mut Value) {
	let object = value.as_object_mut().unwrap();
	let operation = object.remove("operation").unwrap();
	let outcome = operation["outcome"].as_str().unwrap();
	object.insert(
		"status".into(),
		json!(if outcome == "succeeded" {
			"completed"
		} else {
			outcome
		}),
	);
	object.remove("lifecycle");
	object.remove("generation");
	object.remove("removal");
	object.insert("version".into(), json!(1));
}

#[test]
fn legacy_documents_observe_without_writes_and_recover_with_mixed_versions() {
	let fixture = tempfile::tempdir().unwrap();
	let source = repo(fixture.path(), "api");
	let root = fixture.path().join("workspace");
	init_workspace(&root);
	create(&root, &source, "feat/legacy");
	let state = root.join(".workyard");
	let name = workyard_state::task_filename("feat/legacy");
	let record = state.join("tasks").join(&name);
	let intent = state.join("evidence").join(&name);
	let effects = intent.with_extension("effects");
	for path in [&record, &intent] {
		let mut value: Value = serde_json::from_slice(&fs::read(path).unwrap()).unwrap();
		legacy_task(&mut value);
		fs::write(path, serde_json::to_vec(&value).unwrap()).unwrap();
	}
	let mut effect: Value = serde_json::from_slice(&fs::read(&effects).unwrap()).unwrap();
	legacy_task(&mut effect["task"]);
	fs::write(&effects, serde_json::to_vec(&effect).unwrap()).unwrap();

	let before = snapshot(fixture.path());
	let result = rpc(&root, "status", json!({}));
	let task = &result["result"]["tasks"][0];
	assert_eq!(task["lifecycle"], "active");
	assert_eq!(task["condition"], "healthy");
	assert_eq!(task["recorded"]["operation"]["outcome"], "succeeded");
	assert!(task["recorded"]["operation"]["kind"].is_null());
	for command in ["list", "status"] {
		let output = workyard(&root, &[command]);
		assert_ok(&output);
		let text = String::from_utf8(output.stdout).unwrap();
		assert!(!text.contains("succeeded"), "{text}");
		assert!(!text.contains("completed"), "{text}");
		assert!(!text.contains('\u{1b}'), "redirected output must be plain");
	}
	assert_eq!(before, snapshot(fixture.path()));

	assert_ok(&workyard(&root, &["recover", "feat/legacy"]));
	let saved: Value = serde_json::from_slice(&fs::read(&record).unwrap()).unwrap();
	assert_eq!(saved["version"], 3);
	assert_eq!(saved["lifecycle"], "active");
	assert_eq!(saved["operation"]["kind"], "recover");
	assert!(saved["operation"]["continues"].is_null());
	assert!(saved.get("status").is_none());
	assert_eq!(
		serde_json::from_slice::<Value>(&fs::read(&intent).unwrap()).unwrap()["version"],
		1
	);
	assert_ok(&workyard(&root, &["recover", "feat/legacy"]));
}

#[test]
fn clean_worktrees_do_not_hide_coordination_uncertainty_and_modified_is_healthy() {
	let fixture = tempfile::tempdir().unwrap();
	let source = repo(fixture.path(), "api");
	let root = fixture.path().join("workspace");
	init_workspace(&root);
	create(&root, &source, "feat/task");
	fs::write(root.join("tasks/feat/task/api/file"), "modified").unwrap();
	let observed = rpc(&root, "status", json!({}));
	assert_eq!(observed["result"]["tasks"][0]["condition"], "healthy");
	assert_eq!(
		observed["result"]["tasks"][0]["current"][0]["state"],
		"modified"
	);
	let listed = rpc(&root, "list", json!({}));
	assert_eq!(listed["result"]["tasks"][0]["condition"], "healthy");
	assert_eq!(listed["result"]["tasks"][0]["provenance"], "inspected");

	let state = root.join(".workyard");
	let name = workyard_state::task_filename("feat/task");
	let lock = workyard_state::Lock::acquire(
		&state.join("locks").join(&name).with_extension("lock"),
		false,
	)
	.unwrap();
	assert_eq!(
		rpc(&root, "status", json!({}))["result"]["tasks"][0]["condition"],
		"unknown"
	);
	drop(lock);
	let path = state.join("evidence").join(name).with_extension("effects");
	let mut effect: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
	effect["pending"] = json!(0);
	fs::write(&path, serde_json::to_vec(&effect).unwrap()).unwrap();
	assert_eq!(
		rpc(&root, "status", json!({}))["result"]["tasks"][0]["condition"],
		"unknown"
	);
}

#[test]
fn missing_member_is_distinct_from_unavailable_identity_and_remains_managed() {
	let fixture = tempfile::tempdir().unwrap();
	let source = repo(fixture.path(), "infra");
	let root = fixture.path().join("workspace");
	init_workspace(&root);
	create(&root, &source, "feat/task");
	let tree = root.join("tasks/feat/task/infra");
	fs::remove_dir_all(&tree).unwrap();
	let before = snapshot(fixture.path());
	let result = rpc(&root, "status", json!({}));
	let task = &result["result"]["tasks"][0];
	assert_eq!(task["lifecycle"], "active");
	assert_eq!(task["condition"], "needs_attention");
	assert_eq!(task["current"][0]["name"], "infra");
	assert_eq!(task["current"][0]["state"], "missing");
	assert_eq!(task["current"][0]["problem"]["code"], "repository_changed");
	assert_eq!(before, snapshot(fixture.path()));

	fs::create_dir(&tree).unwrap();
	let result = rpc(&root, "status", json!({}));
	assert_eq!(
		result["result"]["tasks"][0]["current"][0]["state"],
		"unavailable"
	);
}

#[test]
fn successful_record_does_not_hide_conflicting_effects_or_condemn_intent_fallback() {
	let fixture = tempfile::tempdir().unwrap();
	let source = repo(fixture.path(), "api");
	let root = fixture.path().join("workspace");
	init_workspace(&root);
	create(&root, &source, "feat/task");
	let path = root
		.join(".workyard/evidence")
		.join(workyard_state::task_filename("feat/task"))
		.with_extension("effects");
	let original: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
	assert_eq!(
		rpc(&root, "status", json!({}))["result"]["tasks"][0]["condition"],
		"healthy"
	);

	for failed in [false, true] {
		let mut effect = original.clone();
		if failed {
			effect["task"]["operation"]["outcome"] = json!("failed");
		} else {
			effect["task"]["participants"][0]["phase"] = json!("creating");
		}
		fs::write(&path, serde_json::to_vec(&effect).unwrap()).unwrap();
		let before = snapshot(fixture.path());
		assert_eq!(
			rpc(&root, "status", json!({}))["result"]["tasks"][0]["condition"],
			if failed { "healthy" } else { "needs_attention" }
		);
		assert_eq!(before, snapshot(fixture.path()));
	}

	fs::remove_file(path).unwrap();
	assert_eq!(
		rpc(&root, "status", json!({}))["result"]["tasks"][0]["condition"],
		"healthy"
	);
}

#[test]
fn mismatched_record_filename_is_identified_without_cluttering_healthy_tasks() {
	let fixture = tempfile::tempdir().unwrap();
	let source = repo(fixture.path(), "api");
	let root = fixture.path().join("workspace");
	init_workspace(&root);
	create(&root, &source, "feat/task");
	let healthy = workyard(&root, &["status"]);
	assert!(
		!String::from_utf8(healthy.stdout)
			.unwrap()
			.contains("State record:")
	);

	let tasks = root.join(".workyard/tasks");
	fs::rename(
		tasks.join(workyard_state::task_filename("feat/task")),
		tasks.join("wrong-name.json"),
	)
	.unwrap();
	let before = snapshot(fixture.path());
	let output = workyard(&root, &["status"]);
	let output = String::from_utf8(output.stdout).unwrap();
	assert!(output.contains("wrong-name.json"), "{output}");
	assert!(output.contains("State record:"), "{output}");
	assert_eq!(before, snapshot(fixture.path()));
}

#[test]
fn command_roles_keep_inventory_compact_status_active_and_info_read_only() {
	let fixture = tempfile::tempdir().unwrap();
	let source = repo(fixture.path(), "api");
	let root = fixture.path().join("workspace");
	init_workspace(&root);
	for index in 0..6 {
		let name = format!("closed/{index}");
		create(&root, &source, &name);
		assert_ok(&workyard(&root, &["remove", &name, "--yes"]));
	}
	create(&root, &source, "feat/active");
	fs::write(root.join("tasks/feat/active/api/file"), "modified").unwrap();
	let before = snapshot(fixture.path());

	let listed = rpc(&root, "list", json!({}));
	let tasks = listed["result"]["tasks"].as_array().unwrap();
	assert_eq!(
		tasks
			.iter()
			.filter(|task| task["lifecycle"] == "retired")
			.count(),
		5
	);
	let listing = workyard(&root, &["list"]);
	assert_ok(&listing);
	let listing = String::from_utf8(listing.stdout).unwrap();
	assert!(listing.contains("feat/active [healthy]  api"), "{listing}");
	assert!(!listing.contains("Repository  State"));
	assert!(listing.contains("Recently closed tasks"));

	let status = workyard(&root, &["status"]);
	assert_ok(&status);
	let status = String::from_utf8(status.stdout).unwrap();
	assert!(status.contains("modified"), "{status}");
	assert!(!status.contains("closed/"));
	for output in [&listing, &status] {
		assert!(!output.contains("Workyard "));
		assert!(!output.contains("Root:"));
		assert!(!output.contains("State:"));
	}

	let info = rpc(&root, "info", json!({}));
	assert_eq!(info["result"]["discovery"], "ancestor");
	assert_eq!(info["result"]["active_tasks"], 1);
	assert_eq!(info["result"]["retired_tasks"], 6);
	let workspace = root.canonicalize().unwrap();
	assert_eq!(info["result"]["workspace"], json!(workspace));
	assert_eq!(info["result"]["tasks_root"], json!(workspace.join("tasks")));
	let info = rpc(&root, "info", json!({"workspace":root}));
	assert_eq!(info["result"]["discovery"], "explicit");
	let info = workyard(&root, &["info"]);
	assert_ok(&info);
	let details = String::from_utf8_lossy(&info.stdout);
	assert!(details.contains("State:     .workyard"), "{details}");
	assert!(details.contains("Tasks dir: tasks"), "{details}");
	assert_eq!(before, snapshot(fixture.path()));
}

#[test]
fn rpc_remove_reports_task_scoped_discard_decision_without_changes() {
	let fixture = tempfile::tempdir().unwrap();
	let source = repo(fixture.path(), "api");
	let root = fixture.path().join("workspace");
	init_workspace(&root);
	create(&root, &source, "feat/task");
	fs::write(root.join("tasks/feat/task/api/file"), "modified").unwrap();
	let before = snapshot(fixture.path());
	let response = rpc(&root, "remove", json!({"task":"feat/task", "yes":true}));
	let outcome = &response["error"]["data"];
	assert_eq!(
		outcome["problem"]["code"], "decision_required",
		"{response}"
	);
	assert_eq!(outcome["decision"]["kind"], "discard_task");
	assert_eq!(outcome["decision"]["task"], "feat/task");
	assert_eq!(outcome["condition"], "healthy");
	assert_eq!(before, snapshot(fixture.path()));
}

#[test]
fn info_preserves_absent_and_damaged_state_while_list_keeps_diagnostics_compact() {
	let fixture = tempfile::tempdir().unwrap();
	let before = snapshot(fixture.path());
	let absent = rpc(fixture.path(), "info", json!({"workspace":"absent"}));
	assert!(absent["result"]["discovery"].is_null());
	assert!(
		absent["result"]["guidance"]
			.as_str()
			.unwrap()
			.contains("workyard init -- absent")
	);
	assert_eq!(before, snapshot(fixture.path()));

	let source = repo(fixture.path(), "api");
	let root = fixture.path().join("workspace");
	init_workspace(&root);
	create(&root, &source, "feat/task");
	let state = root.join(".workyard");
	let path = state
		.join("tasks")
		.join(workyard_state::task_filename("feat/task"));
	fs::write(&path, "invalid json").unwrap();
	let before = snapshot(fixture.path());

	let info = rpc(&root, "info", json!({}));
	assert_eq!(info["result"]["unresolved_records"], 1);
	let output = workyard(&root, &["list"]);
	assert_ok(&output);
	let output = String::from_utf8(output.stdout).unwrap();
	assert!(output.contains("[needs attention]"), "{output}");
	assert!(output.contains("State record:"), "{output}");
	assert!(output.contains("Inspect: workyard status"), "{output}");
	assert!(!output.contains("Invalid JSON"), "{output}");
	assert_eq!(before, snapshot(fixture.path()));
}
