mod support;
use serde_json::Value;
use std::{fs, process::Output};
use support::*;

fn json(output: &Output) -> Value {
	let value: Value = serde_json::from_slice(&output.stdout).unwrap_or_else(|error| {
		panic!(
			"{error}: stdout={} stderr={}",
			String::from_utf8_lossy(&output.stdout),
			String::from_utf8_lossy(&output.stderr)
		)
	});
	assert_eq!(value["exit_code"], output.status.code().unwrap());
	assert!(output.stdout.ends_with(b"\n"));
	assert!(!output.stdout.contains(&b'\t'));
	value
}

#[test]
fn json_inventory_and_argument_failures_are_single_documents() {
	let root = tempfile::tempdir().unwrap();
	for args in [["--json", "list"], ["list", "--json"]] {
		let output = workyard(root.path(), &args);
		assert_ok(&output);
		assert!(json(&output)["error"].is_null());
		assert!(output.stderr.is_empty());
		assert!(!root.path().join(".workyard").exists());
	}
	for args in [
		vec!["--json", "not-a-command"],
		vec!["list", "--json", "--no-such-option"],
		vec!["--json", "remove"],
	] {
		let output = workyard(root.path(), &args);
		assert!(!output.status.success());
		let value = json(&output);
		assert!(value["result"].is_null());
		assert_eq!(value["error"]["code"], "invalid_input");
		assert!(output.stderr.is_empty());
	}
}

#[test]
fn json_creation_preserves_grouped_repositories_and_domain_failure() {
	let root = tempfile::tempdir().unwrap();
	let api = repo(root.path(), "api");
	let web = repo(root.path(), "web");
	let args = [
		"checkout",
		"feat/payments",
		"--repo",
		text(&api),
		"--repo-name",
		"backend",
		"--json",
		"--repo",
		text(&web),
		"--repo-name",
		"frontend",
		"--no-fetch",
		"--from",
		"main",
	];
	let created = workyard(root.path(), &args);
	assert_ok(&created);
	let value = json(&created);
	assert_eq!(value["result"]["lifecycle"], "active");
	assert_eq!(value["result"]["participants"][0]["name"], "backend");
	assert_eq!(value["result"]["participants"][1]["name"], "frontend");
	assert!(created.stderr.is_empty());
	assert!(root.path().join("feat/payments/backend/.git").exists());

	let duplicate = workyard(root.path(), &args);
	assert!(!duplicate.status.success());
	let value = json(&duplicate);
	assert!(value["error"].is_null());
	assert_eq!(value["result"]["operation"]["outcome"], "failed");
	assert!(value["result"]["problem"]["code"].is_string());
}

#[test]
fn json_remove_never_implies_consent_or_damages_usable_state() {
	let root = tempfile::tempdir().unwrap();
	let api = repo(root.path(), "api");
	assert_ok(&workyard(
		root.path(),
		&[
			"create",
			"feat/payments",
			"--repo",
			text(&api),
			"--no-fetch",
			"--from",
			"main",
		],
	));
	let participant = root.path().join("feat/payments/api");
	fs::write(participant.join("file"), "uncommitted work\n").unwrap();
	let before = record(&root.path().join(".workyard"));

	let output = workyard(root.path(), &["--json", "remove", "feat/payments"]);
	assert!(!output.status.success());
	assert!(json(&output)["error"].is_null());
	assert!(output.stderr.is_empty());
	assert_eq!(record(&root.path().join(".workyard")), before);
	assert_eq!(
		fs::read_to_string(participant.join("file")).unwrap(),
		"uncommitted work\n"
	);

	let forced = workyard(
		root.path(),
		&["remove", "feat/payments", "--json", "--force"],
	);
	assert_ok(&forced);
	assert!(json(&forced)["error"].is_null());
	assert!(!participant.exists());
	assert_eq!(
		git(&api, &["branch", "--list", "feat/payments"]),
		"feat/payments"
	);
}

#[test]
fn completion_scripts_cover_registered_actions_aliases_and_host_commands() {
	let root = tempfile::tempdir().unwrap();
	for shell in ["bash", "zsh", "fish", "powershell"] {
		let output = workyard(root.path(), &["completion", shell]);
		assert_ok(&output);
		let script = String::from_utf8(output.stdout).unwrap();
		assert!(script.contains("workyard"));
		assert!(script.contains("TRINE_COMPLETE"));
		assert!(!script.contains("Invoke-Expression"));
		assert!(!script.contains("bashdefault"));
		assert!(output.stderr.is_empty());
	}
	assert!(fs::read_dir(root.path()).unwrap().next().is_none());
}

#[test]
fn display_commands_stay_plain_and_json_rejects_transports_and_scripts() {
	let root = tempfile::tempdir().unwrap();
	for args in [
		vec!["--json", "--help"],
		vec!["list", "--json", "--help"],
		vec!["--json", "--version"],
	] {
		let output = workyard(root.path(), &args);
		assert_ok(&output);
		assert!(serde_json::from_slice::<Value>(&output.stdout).is_err());
	}
	for args in [
		vec!["--json", "rpc"],
		vec!["rpc", "--json"],
		vec!["completion", "bash", "--json"],
	] {
		let output = workyard(root.path(), &args);
		assert!(!output.status.success());
		assert_eq!(json(&output)["error"]["code"], "invalid_input");
	}
}

#[test]
fn optional_pull_failure_is_an_invocation_result_not_damaged_membership() {
	let root = tempfile::tempdir().unwrap();
	let (api, _) = remote_clone(root.path(), "api");
	let output = workyard(
		root.path(),
		&[
			"create",
			"feat/payments",
			"--repo",
			text(&api),
			"--from",
			"main",
			"--pull",
			"--json",
		],
	);
	assert!(!output.status.success());
	let value = json(&output);
	assert!(value["error"].is_null());
	assert_eq!(value["result"]["pull_failures"][0]["participant"], "api");
	assert_eq!(value["result"]["participants"][0]["phase"], "completed");
	assert_eq!(value["result"]["lifecycle"], "active");
	assert!(output.stderr.is_empty());
	let saved = record(&root.path().join(".workyard"));
	assert!(saved["problem"].is_null());
	assert!(saved.get("pull_failures").is_none());
}

#[test]
fn populated_status_and_info_json_preserve_their_distinct_facts() {
	let root = tempfile::tempdir().unwrap();
	let api = repo(root.path(), "api");
	let created = workyard(
		root.path(),
		&[
			"create",
			"feat/payments",
			"--repo",
			text(&api),
			"--from",
			"main",
			"--no-fetch",
		],
	);
	assert_ok(&created);
	fs::write(root.path().join("feat/payments/api/file"), "modified\n").unwrap();

	let status = workyard(root.path(), &["status", "feat/payments", "--json"]);
	assert_ok(&status);
	let value = json(&status);
	assert!(value["error"].is_null());
	let tasks = value["result"]["tasks"].as_array().unwrap();
	assert_eq!(tasks.len(), 1);
	assert_eq!(tasks[0]["name"], "feat/payments");
	assert_eq!(tasks[0]["condition"], "healthy");
	assert_eq!(tasks[0]["current"][0]["name"], "api");
	assert_eq!(tasks[0]["current"][0]["state"], "modified");
	assert_eq!(tasks[0]["current"][0]["dirty"], true);
	assert!(status.stderr.is_empty());

	let info = workyard(root.path(), &["--json", "info"]);
	assert_ok(&info);
	let value = json(&info);
	assert!(value["error"].is_null());
	let result = &value["result"];
	assert_eq!(result["discovery"], "ancestor");
	assert_eq!(result["active_tasks"], 1);
	assert_eq!(result["retired_tasks"], 0);
	assert_eq!(result["unresolved_records"], 0);
	assert_eq!(result["busy_tasks"], 0);
	assert!(result["tasks_root"].is_string());
	assert!(result["state_dir"].is_string());
	assert!(info.stderr.is_empty());
}

#[test]
fn task_help_distinguishes_checkout_and_workspace_recovery() {
	let root = tempfile::tempdir().unwrap();
	let help = |command: &str| {
		let output = workyard(root.path(), &[command, "--help"]);
		assert_ok(&output);
		String::from_utf8(output.stdout).unwrap()
	};

	let create = help("create");
	assert!(create.contains("<TASK>"));
	assert!(create.contains("--checkout"));

	let checkout = help("checkout");
	assert!(checkout.contains("reusing existing target branches"));
	assert!(!checkout.contains("--checkout"));

	let recover = help("recover");
	assert!(recover.contains("[TASK]"));
	assert!(!recover.contains("[TASK]..."));
	assert!(recover.contains("Resume workspace relocation"));
}
