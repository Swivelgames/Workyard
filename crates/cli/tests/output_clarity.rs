#[allow(dead_code)]
mod support;

use std::fs;
use support::*;

#[test]
fn empty_workspace_messages_belong_at_workspace_level() {
	let fixture = tempfile::tempdir().unwrap();
	for action in ["list", "status"] {
		let output = workyard(fixture.path(), &[action]);
		assert_ok(&output);
		let output = String::from_utf8(output.stdout).unwrap();
		assert!(
			output.contains("No Workyard state found.\nGet started: workyard create"),
			"{output}"
		);
		assert!(!output.contains("        No Workyard"), "{output}");
		assert!(!output.ends_with("\n\n"), "{output}");
	}

	let output = workyard(fixture.path(), &["status", "--state-dir", "absent"]);
	assert_ok(&output);
	let output = String::from_utf8(output.stdout).unwrap();
	assert!(output.contains("--state-dir=absent"), "{output}");
}

#[test]
fn summaries_omit_repetition_but_keep_branch_exceptions() {
	let fixture = tempfile::tempdir().unwrap();
	let source = repo(fixture.path(), "api");
	let root = fixture.path().join("workspace");
	fs::create_dir(&root).unwrap();

	let output = workyard(
		&root,
		&[
			"create",
			"--no-fetch",
			"--from",
			"main",
			"--repo",
			text(&source),
			"feat/example",
		],
	);
	assert_ok(&output);
	let output = String::from_utf8(output.stdout).unwrap();
	assert!(output.contains("api         created\n"), "{output}");
	assert!(!output.contains("steps finished"), "{output}");

	let output = workyard(&root, &["list"]);
	assert_ok(&output);
	let output = String::from_utf8(output.stdout).unwrap();
	assert!(output.contains("feat/example [healthy]  api"), "{output}");
	assert!(!output.contains("Repository  State"), "{output}");
	assert!(!output.contains("Recorded participants"), "{output}");

	let output = workyard(&root, &["status", "feat/example"]);
	assert_ok(&output);
	assert!(
		!String::from_utf8(output.stdout)
			.unwrap()
			.contains("api branch:")
	);

	git(&root.join("feat/example/api"), &["checkout", "-b", "other"]);
	let output = workyard(&root, &["status", "feat/example"]);
	assert_ok(&output);
	let output = String::from_utf8(output.stdout).unwrap();
	assert!(output.contains("api branch: other"), "{output}");
}

#[test]
fn workspace_errors_are_not_indented_as_participant_diagnostics() {
	let fixture = tempfile::tempdir().unwrap();
	let state = fixture.path().join(".workyard");
	fs::create_dir(&state).unwrap();
	fs::write(state.join("workspace.lock"), "").unwrap();
	fs::write(state.join("workspace.json"), "invalid JSON").unwrap();

	let output = workyard(fixture.path(), &["status"]);
	assert!(!output.status.success());
	let output = String::from_utf8(output.stdout).unwrap();
	assert!(output.starts_with("damaged_state:"), "{output}");
	assert!(!output.contains("        damaged_state:"), "{output}");
}
