#[allow(dead_code)]
mod support;

use serde_json::{Value, json};
use std::{
	fs,
	io::Write,
	path::Path,
	process::{Command, Stdio},
};
use support::{assert_ok, repo, text, workyard};

#[test]
fn bare_init_targets_cwd_and_repeating_preserves_workspace_state() {
	let fixture = tempfile::tempdir().unwrap();
	let root = fixture.path();
	let first = workyard(root, &["init"]);
	assert_ok(&first);
	assert!(String::from_utf8_lossy(&first.stdout).contains("Workspace ready"));
	assert!(root.join("tasks").is_dir());
	assert!(!root.join("projects").exists());

	let record = root.join(".workyard/workspace.json");
	let prior = fs::read(&record).unwrap();
	fs::write(root.join("tasks/notes"), b"keep me").unwrap();

	assert_ok(&workyard(root, &["init", "."]));
	assert_eq!(fs::read(record).unwrap(), prior);
	assert_eq!(fs::read(root.join("tasks/notes")).unwrap(), b"keep me");
}

#[test]
fn rpc_and_cli_use_the_same_explicit_target_and_relative_tasks_path() {
	let fixture = tempfile::tempdir().unwrap();
	let input = json!({"directory": "workspace", "tasks": "nested/worktrees"});
	let rpc = invoke_init(fixture.path(), input);
	let workspace = fixture.path().join("workspace").canonicalize().unwrap();
	let tasks = workspace.join("nested/worktrees");
	assert!(tasks.is_dir());
	assert_eq!(rpc["result"]["workspace"], json!(workspace));
	assert_eq!(
		rpc["result"]["tasks_root"],
		json!(tasks.canonicalize().unwrap())
	);

	let cli = workyard(
		fixture.path(),
		&["init", "workspace", "--tasks", "nested/worktrees", "--json"],
	);
	assert_ok(&cli);
	let json: Value = serde_json::from_slice(&cli.stdout).unwrap();
	assert_eq!(json["result"], rpc["result"]);
	assert!(!fixture.path().join(".workyard").exists());
}

#[test]
fn create_from_a_descendant_uses_the_configured_tasks_directory() {
	let fixture = tempfile::tempdir().unwrap();
	let workspace = fixture.path().join("workspace");
	let api = repo(fixture.path(), "api");
	assert_ok(&workyard(
		fixture.path(),
		&["init", "workspace", "--tasks", "dev/trees"],
	));

	let descendant = workspace.join("dev");
	let create = workyard(
		&descendant,
		&[
			"create",
			"feat/example",
			"--repo",
			text(&api),
			"--no-fetch",
			"--from",
			"main",
		],
	);
	assert_ok(&create);
	let participant = workspace.join("dev/trees/feat/example/api");
	assert!(participant.join(".git").is_file());
	assert!(!descendant.join(".workyard").exists());

	let inventory = workyard(&participant, &["list", "--json"]);
	assert_ok(&inventory);
	let response: Value = serde_json::from_slice(&inventory.stdout).unwrap();
	assert_eq!(response["result"]["tasks"][0]["name"], "feat/example");
}

#[test]
fn conflicting_reinit_fails_without_reconfiguring_the_workspace() {
	let fixture = tempfile::tempdir().unwrap();
	let root = fixture.path();
	assert_ok(&workyard(root, &["init", "--tasks", "worktrees"]));
	let record = root.join(".workyard/workspace.json");
	let original = fs::read(&record).unwrap();

	let mismatch = workyard(root, &["init", "--tasks", "other", "--json"]);
	assert!(!mismatch.status.success());
	let response: Value = serde_json::from_slice(&mismatch.stdout).unwrap();
	assert_eq!(response["result"]["code"], "workspace_binding_conflict");
	assert!(
		response["result"]["message"]
			.as_str()
			.unwrap()
			.contains("worktrees")
	);
	assert_eq!(fs::read(record).unwrap(), original);
	assert!(!root.join("other").exists());
}

#[test]
fn bare_init_inside_another_workspace_selects_cwd_and_descendants_use_the_nearest_marker() {
	let fixture = tempfile::tempdir().unwrap();
	let outer = fixture.path();
	assert_ok(&workyard(outer, &["init"]));
	let inner = outer.join("inner");
	fs::create_dir(&inner).unwrap();

	assert_ok(&workyard(&inner, &["init", "--tasks", "dev/trees"]));
	let tasks = inner.join("dev/trees");
	let info = workyard(&tasks, &["info", "--json"]);
	assert_ok(&info);
	let response: Value = serde_json::from_slice(&info.stdout).unwrap();
	assert_eq!(
		response["result"]["workspace"],
		json!(inner.canonicalize().unwrap())
	);
	assert_eq!(
		response["result"]["tasks_root"],
		json!(tasks.canonicalize().unwrap())
	);
}

#[cfg(unix)]
#[test]
fn cli_keeps_native_directory_names_lossless() {
	use std::{ffi::OsStr, os::unix::ffi::OsStrExt};

	let fixture = tempfile::tempdir().unwrap();
	let directory = OsStr::from_bytes(b"workspace-\xff");
	let result = Command::new(env!("CARGO_BIN_EXE_workyard"))
		.current_dir(fixture.path())
		.arg("init")
		.arg(directory)
		.arg("--json")
		.output()
		.unwrap();
	assert_ok(&result);
	let outcome: Value = serde_json::from_slice(&result.stdout).unwrap();
	assert_eq!(outcome["result"]["workspace"]["platform"], "unix");
	assert!(fixture.path().join(directory).join("tasks").is_dir());
}

fn invoke_init(cwd: &Path, input: Value) -> Value {
	let mut host = Command::new(env!("CARGO_BIN_EXE_workyard"))
		.current_dir(cwd)
		.arg("rpc")
		.stdin(Stdio::piped())
		.stdout(Stdio::piped())
		.spawn()
		.unwrap();
	let request = json!({
		"jsonrpc": "2.0",
		"id": 1,
		"method": "invoke",
		"params": { "action": ["init"], "input": input }
	});
	writeln!(host.stdin.take().unwrap(), "{request}").unwrap();

	let output = host.wait_with_output().unwrap();
	assert_ok(&output);
	serde_json::from_slice(&output.stdout).unwrap()
}
