#[allow(dead_code)]
mod support;

use std::{fs, path::Path};
use support::*;

#[test]
fn empty_workspace_messages_belong_at_workspace_level() {
	let fixture = tempfile::tempdir().unwrap();
	for action in ["list", "status"] {
		let output = workyard(fixture.path(), &[action]);
		assert_ok(&output);
		let output = human_body(&output);
		assert!(
			output.starts_with("  No Workyard workspace found. Get started: workyard init"),
			"{output}"
		);
		assert!(!output.contains("        No Workyard"), "{output}");
		assert!(!output.ends_with("\n\n"), "{output}");
	}

	let output = workyard(fixture.path(), &["status", "--workspace", "absent"]);
	assert_ok(&output);
	let output = String::from_utf8(output.stdout).unwrap();
	assert!(output.contains("workyard init -- absent"), "{output}");
}

#[test]
fn summaries_omit_repetition_but_keep_branch_exceptions() {
	let fixture = tempfile::tempdir().unwrap();
	let source = repo(fixture.path(), "api");
	let root = fixture.path().join("workspace");
	init_workspace(&root);

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
	let progress = String::from_utf8_lossy(&output.stderr);
	assert_eq!(progress, "    api: creating worktree\n\n");

	let output = human_body(&output);
	assert!(output.starts_with("  Task ready\n\n"), "{output}");
	assert!(!output.contains("Root:"), "{output}");
	assert!(
		output.contains("    feat/example [active]\n\n      Repository"),
		"{output}"
	);
	assert!(output.contains("      api         created\n"), "{output}");
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

	git(
		&root.join("tasks/feat/example/api"),
		&["checkout", "-b", "other"],
	);
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
	let output = human_body(&output);
	assert!(output.starts_with("  "), "{output}");
	assert!(!output.starts_with("    "), "{output}");
	assert!(!output.contains("damaged_state:"), "{output}");
	assert!(output.contains("Invalid JSON"), "{output}");
	assert!(output.contains("workspace.json"), "{output}");
}

#[test]
fn create_and_checkout_explain_branch_conflicts_without_raw_git_diagnostics() {
	let fixture = tempfile::tempdir().unwrap();
	let root = fixture.path();
	let projects = root.join("projects");
	fs::create_dir(&projects).unwrap();
	let api = repo(&projects, "api");
	let infra = repo(&projects, "infra");
	for repository in [&api, &infra] {
		git(repository, &["branch", "fix/emptiness"]);
	}
	assert_ok(&workyard(root, &["init"]));

	let args = ["fix/emptiness", "--from", "main", "api", "infra"];
	let output = workyard(root, &[&["create"][..], &args].concat());
	assert!(!output.status.success());
	let output = human_body(&output);
	let api_label = Path::new("projects").join("api");
	let infra_label = Path::new("projects").join("infra");
	assert!(output.contains("Could not start fix/emptiness"), "{output}");
	assert!(
		output.contains(&format!(
			"Branch already exists in {}, {}.",
			text(&api_label),
			text(&infra_label)
		)),
		"{output}"
	);
	assert!(
		output.contains(
			"  Try checkout instead\n\n    workyard checkout --from=main fix/emptiness api infra"
		),
		"{output}"
	);
	assert!(!output.contains("branch_exists:"), "{output}");
	assert!(!output.contains("[not inspected]"), "{output}");

	git(&api, &["checkout", "fix/emptiness"]);
	let output = workyard(root, &[&["checkout"][..], &args].concat());
	assert!(!output.status.success());
	let output = human_body(&output);
	assert!(
		output.contains(&format!(
			"Branch is already checked out in {}.",
			text(&api_label)
		)),
		"{output}"
	);
	assert!(output.contains("git worktree prune"), "{output}");
	assert!(!output.contains("--porcelain"), "{output}");
	assert!(!output.contains("branch_in_use:"), "{output}");
	assert!(!root.join("tasks/fix/emptiness").exists());

	let json = workyard(root, &[&["checkout", "--json"][..], &args].concat());
	assert!(!json.status.success());
	let json = serde_json::from_slice::<serde_json::Value>(&json.stdout).unwrap();
	let problem = &json["result"]["problem"];
	assert_eq!(problem["code"], "branch_in_use");
	assert!(
		problem["message"]
			.as_str()
			.unwrap()
			.contains(&format!("{:?}", api.canonicalize().unwrap()))
	);
}

#[test]
fn fetch_failure_shows_a_short_git_diagnostic_and_keeps_full_structured_details() {
	let fixture = tempfile::tempdir().unwrap();
	let root = fixture.path();
	init_workspace(root);
	let projects = root.join("projects");
	fs::create_dir(&projects).unwrap();
	let api = repo(&projects, "api");
	#[cfg(unix)]
	let missing_remote = std::path::PathBuf::from(format!(
		"{}/outside dir {}/missing-remote",
		root.display(),
		root.display()
	));
	#[cfg(not(unix))]
	let missing_remote = root.join("missing-remote");
	git(&api, &["remote", "add", "origin", text(&missing_remote)]);

	let output = workyard(root, &["create", "feat/human", "api", "--from", "main"]);
	assert!(!output.status.success());
	let body = human_body(&output);
	let api_label = Path::new("projects").join("api");
	assert!(
		body.contains(&format!("Git failed in {}.", text(&api_label))),
		"{body}"
	);
	assert!(body.contains("missing-remote"), "{body}");
	assert!(
		body.contains("Git details: workyard status --json -- feat/human"),
		"{body}"
	);
	#[cfg(unix)]
	assert!(body.contains(text(&missing_remote)), "{body}");
	assert!(!body.contains("git_failed:"), "{body}");
	assert!(!body.contains("git ["), "{body}");
	assert!(
		body.contains("Inspect: workyard status -- feat/human"),
		"{body}"
	);
	assert!(!body.contains("--workspace"), "{body}");

	let status = workyard(root, &["status", "feat/human"]);
	assert_ok(&status);
	let body = human_body(&status);
	assert!(body.contains("not created"), "{body}");
	assert!(body.contains("missing-remote"), "{body}");

	let explicit = workyard(
		root,
		&[
			"checkout",
			"feat/explicit",
			"api",
			"--from",
			"main",
			"--workspace",
			".",
		],
	);
	assert!(!explicit.status.success());
	let body = human_body(&explicit);
	assert!(
		body.contains("Inspect: workyard status --workspace=. -- feat/explicit"),
		"{body}"
	);

	let output = workyard(
		root,
		&["create", "feat/machine", "api", "--from", "main", "--json"],
	);
	assert!(!output.status.success());
	let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
	let problem = &report["result"]["problem"];
	assert_eq!(problem["code"], "git_failed");
	assert!(
		problem["message"]
			.as_str()
			.unwrap()
			.contains("missing-remote")
	);
	assert!(problem.get("detail").is_none());
}

#[test]
fn follow_up_commands_keep_the_explicit_workspace() {
	let fixture = tempfile::tempdir().unwrap();
	let a = fixture.path().join("a");
	let b = fixture.path().join("b");
	init_workspace(&a);
	init_workspace(&b);

	for action in ["status", "remove", "recover"] {
		let output = workyard(&a, &[action, "--workspace", "../b", "unknown"]);
		assert!(!output.status.success());
		let body = human_body(&output);
		assert!(body.contains("workyard list --workspace=../b"), "{body}");
	}
	assert_ok(&workyard(
		&a,
		&[
			"projects",
			"untrack",
			"--workspace",
			"../b",
			"../b/projects",
		],
	));
	let output = workyard(&a, &["projects", "list", "--workspace", "../b"]);
	assert_ok(&output);
	let body = human_body(&output);
	assert!(
		body.contains("workyard projects track --workspace=../b <path>"),
		"{body}"
	);
	assert!(body.contains("Replace <path>"), "{body}");
}

#[test]
fn recorded_setup_problems_offer_recoverable_next_steps() {
	let fixture = tempfile::tempdir().unwrap();
	let origin = repo(fixture.path(), "origin");
	let root = fixture.path().join("workspace");
	init_workspace(&root);
	let projects = root.join("projects");
	fs::create_dir(&projects).unwrap();
	git(&projects, &["clone", text(&origin), "api"]);

	let output = workyard(
		&root,
		&["create", "feat/missing", "api", "--from", "origin/missing"],
	);
	assert!(!output.status.success());
	let body = human_body(&output);
	assert!(body.contains("Recovery uses the recorded source"), "{body}");
	assert!(body.contains("workyard recover feat/missing"), "{body}");
	assert!(!body.contains("--repo-from"), "{body}");

	let recovery = workyard(&root, &["recover", "feat/missing"]);
	assert!(!recovery.status.success());
	let body = human_body(&recovery);
	assert!(body.contains("Recovery uses the recorded source"), "{body}");
	assert!(!body.contains("--repo-from"), "{body}");

	git(&origin, &["branch", "feat/conflict"]);
	let output = workyard(&root, &["create", "feat/conflict", "api"]);
	assert!(!output.status.success());
	let body = human_body(&output);
	assert!(
		body.contains("This task already has a saved plan"),
		"{body}"
	);
	assert!(
		body.contains("Use a different task name to start fresh"),
		"{body}"
	);
	assert!(!body.contains("Try checkout instead"), "{body}");
}
