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
fn cli_and_rpc_reject_conflicting_modes_before_discovery() {
	let fixture = tempfile::tempdir().unwrap();
	let cwd = fixture.path();
	let cli = workyard(cwd, &["relocate", "--keep-source", "--move", "--json"]);
	assert!(!cli.status.success());
	let cli: Value = serde_json::from_slice(&cli.stdout).unwrap();
	let rpc = rpc(cwd, "relocate", json!({"keep_source": true, "move": true}));
	assert_eq!(cli["result"]["problem"]["code"], "invalid_input");
	assert_eq!(cli["result"], rpc["error"]["data"]);
	assert_eq!(cli["result"]["mutation_started"], false);
	assert_eq!(cwd.read_dir().unwrap().count(), 0);
}

#[test]
fn rpc_copy_and_cli_inspection_agree_on_the_retained_snapshot() {
	let fixture = tempfile::tempdir().unwrap();
	let cwd = fixture.path().canonicalize().unwrap();
	let source = cwd.join("workspace");
	let destination = cwd.join("new workspace");
	let backing = repo(&cwd, "api");
	fs::create_dir(&source).unwrap();
	assert_ok(&workyard(
		&source,
		&[
			"create",
			"feat/one",
			"--repo",
			text(&backing),
			"--no-fetch",
			"--from",
			"main",
		],
	));

	let response = rpc(
		&cwd,
		"relocate",
		json!({"copy": true, "old": source, "new": destination}),
	);
	let result = &response["result"];
	assert_eq!(result["copy"], true, "{response}");
	let snapshot = &result["snapshot"];
	assert_eq!(snapshot["partial"], false);
	assert_eq!(snapshot["tasks_root"], json!(source));
	assert_eq!(snapshot["active_tasks_root"], json!(destination));

	for action in ["list", "status", "info"] {
		let cli = workyard(&source, &[action, "--json"]);
		assert_ok(&cli);
		let observed: Value = serde_json::from_slice(&cli.stdout).unwrap();
		assert_eq!(observed["result"]["snapshot"], *snapshot, "{action}");

		let human = workyard(&source, &[action]);
		assert_ok(&human);
		assert!(String::from_utf8_lossy(&human.stdout).contains("Inactive snapshot"));
	}
	assert_snapshot_guards(&source, &destination, &backing);
}

fn assert_snapshot_guards(source: &Path, destination: &Path, backing: &Path) {
	let state = source.join(".workyard");
	let other = source.parent().unwrap().join("unused");
	let task = "feat/one";
	for args in [
		vec!["create", "feat/new", "--repo", text(backing)],
		vec!["checkout", "feat/new", "--repo", text(backing)],
		vec![
			"create",
			"feat/new",
			"--repo",
			text(backing),
			"--tasks-root",
			text(&other),
			"--state-dir",
			text(&state),
		],
		vec!["amend", task, "--repo", text(backing)],
		vec!["remove", task, "--force"],
		vec!["recover", task],
		vec!["recover"],
		vec!["relocate", "--old", text(source), "--new", text(&other)],
	] {
		let output = workyard(source, &args);
		let message = String::from_utf8_lossy(&output.stdout);
		assert!(!output.status.success(), "{args:?}");
		assert!(message.contains("inactive snapshot"), "{args:?}: {message}");
		assert!(message.contains("new workspace"), "{args:?}: {message}");
	}
	assert!(!other.exists());
	assert!(destination.join("feat/one/api/file").is_file());

	let completion = Command::new(env!("CARGO_BIN_EXE_workyard"))
		.current_dir(source)
		.env("TRINE_COMPLETE", "zsh")
		.env("_CLAP_COMPLETE_INDEX", "2")
		.env_remove("TRINE_COMPLETE_LINE")
		.args(["--", "workyard", "amend", "feat/"])
		.output()
		.unwrap();
	assert_ok(&completion);
	assert!(!String::from_utf8_lossy(&completion.stdout).contains(task));
	assert!(completion.stderr.is_empty());
}

fn rpc(cwd: &Path, action: &str, input: Value) -> Value {
	let mut child = Command::new(env!("CARGO_BIN_EXE_workyard"))
		.current_dir(cwd)
		.arg("rpc")
		.stdin(Stdio::piped())
		.stdout(Stdio::piped())
		.spawn()
		.unwrap();
	let request = json!({
		"jsonrpc": "2.0", "id": 1, "method": "invoke",
		"params": {"action": [action], "input": input}
	});
	writeln!(child.stdin.take().unwrap(), "{request}").unwrap();
	let response = child.wait_with_output().unwrap();
	assert_ok(&response);
	serde_json::from_slice(&response.stdout).unwrap()
}
