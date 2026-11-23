#[allow(dead_code)]
mod support;

use serde_json::{Value, json};
use std::{
	fs,
	io::Write,
	path::Path,
	process::{Command, Stdio},
};
use support::{assert_ok, human_body, repo, text, workyard};

#[test]
fn bare_init_targets_cwd_and_repeating_preserves_workspace_state() {
	let fixture = tempfile::tempdir().unwrap();
	let root = fixture.path();
	let first = workyard(root, &["init"]);
	assert_ok(&first);
	let body = human_body(&first);
	assert!(
		body.starts_with("  Workspace initialized\n\n    .workyard/\n    tasks/\n"),
		"{body}"
	);
	assert!(body.contains("workyard projects track ./repos\n"), "{body}");
	assert!(
		body.contains("workyard projects track ./repos/api ./repos/web"),
		"{body}"
	);
	assert!(!body.contains("Found "), "{body}");
	assert!(!body.contains('\t'), "{body}");
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
fn init_previews_existing_projects_and_uses_their_references_in_examples() {
	let fixture = tempfile::tempdir().unwrap();
	let workspace = fixture.path().join("workspace");
	let projects = workspace.join("projects");
	fs::create_dir_all(&projects).unwrap();
	for name in ["zeta", "infra", "api", "docs", "web"] {
		repo(&projects, name);
	}

	let output = workyard(
		fixture.path(),
		&["init", "workspace", "--tasks", "dev/trees"],
	);
	assert_ok(&output);
	let body = human_body(&output);
	let tasks = Path::new("dev").join("trees");
	let preview_path = Path::new("projects").join("api");

	assert!(
		body.contains("Workspace initialized at workspace"),
		"{body}"
	);
	assert!(body.contains(&format!("    {}/", text(&tasks))), "{body}");
	assert!(body.contains("Found 5 projects"), "{body}");
	assert!(
		body.contains(&format!("    {}/", text(&preview_path))),
		"{body}"
	);
	assert!(body.contains("… and 1 more"), "{body}");
	assert!(!body.contains("zeta"), "{body}");
	assert!(
		body.contains("Run these commands from the workspace directory."),
		"{body}"
	);
	assert!(
		body.contains("workyard create feat/my-feature api docs"),
		"{body}"
	);
	assert!(
		body.contains("workyard checkout feat/my-feature api docs"),
		"{body}"
	);
	assert!(workspace.join("dev/trees").is_dir());
	assert!(!workspace.join("dev/trees/feat").exists());
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

	let mismatch = workyard(root, &["init", "--tasks", "other"]);
	assert!(!mismatch.status.success());
	assert_eq!(
		human_body(&mismatch),
		"  Could not initialize workspace\n\n    The tasks directory is already set to worktrees.\n    Drop --tasks to keep the existing layout.\n"
	);

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

#[test]
fn detached_tasks_require_explicit_placement_and_limit_participants_to_tracked_projects() {
	let fixture = tempfile::tempdir().unwrap();
	let root = fixture.path();
	fs::create_dir(root.join("projects")).unwrap();
	repo(&root.join("projects"), "api");
	let outside = repo(root, "outside");

	let invalid = workyard(
		root,
		&[
			"init",
			"provider",
			"--tasks",
			"local",
			"--detached-tasks",
			"../tasks",
		],
	);
	assert!(!invalid.status.success());
	assert!(!root.join("provider").exists());
	let ordinary = workyard(root, &["init", "provider", "--tasks", "../tasks"]);
	assert!(!ordinary.status.success());
	assert!(!root.join("provider").exists());

	let options = [
		"init",
		"provider",
		"--detached-tasks",
		"../tasks",
		"--tracked-only",
		"--track",
		"../projects",
	];
	let initialized = workyard(root, &options);
	assert_ok(&initialized);
	assert!(
		String::from_utf8_lossy(&initialized.stdout).contains("Use --workspace on every command")
	);
	assert_ok(&workyard(root, &options));

	let rejected = workyard(
		root,
		&[
			"create",
			"--workspace",
			"provider",
			"feat/rejected",
			"api",
			outside.to_str().unwrap(),
			"--from",
			"main",
			"--no-fetch",
		],
	);
	assert!(!rejected.status.success());
	assert!(!root.join("tasks/feat/rejected").exists());

	assert_ok(&workyard(
		root,
		&[
			"create",
			"--workspace",
			"provider",
			"feat/demo",
			"api",
			"--from",
			"main",
			"--no-fetch",
		],
	));
	let observed = workyard(
		root,
		&["status", "--workspace", "provider", "feat/demo", "--json"],
	);
	assert_ok(&observed);
	let result: Value = serde_json::from_slice(&observed.stdout).unwrap();
	assert_eq!(result["result"]["tasks"][0]["current"][0]["name"], "api");
	assert!(root.join("tasks/feat/demo/api/.git").is_file());
	assert!(!root.join("provider/tasks").exists());
}

#[test]
fn init_tracks_individual_repositories_and_deep_directories() {
	let fixture = tempfile::tempdir().unwrap();
	let root = fixture.path();
	repo(root, "standalone repo");
	let nested = root.join("code/products/backend");
	fs::create_dir_all(&nested).unwrap();
	repo(&nested, "service");
	fs::write(root.join("file"), "not a directory").unwrap();

	let invalid = workyard(root, &["init", "invalid", "--track", "../file"]);
	assert!(!invalid.status.success());
	assert!(!root.join("invalid").exists());

	let init = workyard(
		root,
		&[
			"init",
			"workspace",
			"--track",
			"../standalone repo",
			"--track",
			"../code",
			"--track",
			"../future",
		],
	);
	assert_ok(&init);

	let inventory = workyard(
		root,
		&["projects", "list", "--workspace", "workspace", "--json"],
	);
	assert_ok(&inventory);
	let inventory: Value = serde_json::from_slice(&inventory.stdout).unwrap();
	let result = &inventory["result"];
	assert_eq!(result["roots"].as_array().unwrap().len(), 3);
	assert!(
		result["roots"]
			.as_array()
			.unwrap()
			.iter()
			.any(|root| root["status"] == "missing")
	);
	let references: Vec<_> = result["projects"]
		.as_array()
		.unwrap()
		.iter()
		.filter_map(|project| project["reference"].as_str())
		.collect();
	assert_eq!(references, ["service", "standalone repo"]);

	assert_ok(&workyard(
		root,
		&[
			"init",
			"workspace",
			"--track",
			"../future",
			"--track",
			"../code",
			"--track",
			"../standalone repo",
		],
	));

	assert_ok(&workyard(
		root,
		&["projects", "track", "--workspace", "workspace", "./code"],
	));
	let inventory = workyard(
		root,
		&["projects", "list", "--workspace", "workspace", "--json"],
	);
	assert_ok(&inventory);
	let inventory: Value = serde_json::from_slice(&inventory.stdout).unwrap();
	assert_eq!(inventory["result"]["roots"].as_array().unwrap().len(), 3);

	assert_ok(&workyard(
		root,
		&["projects", "untrack", "--workspace", "workspace", "./code"],
	));
	let inventory = workyard(
		root,
		&["projects", "list", "--workspace", "workspace", "--json"],
	);
	assert_ok(&inventory);
	let inventory: Value = serde_json::from_slice(&inventory.stdout).unwrap();
	assert_eq!(inventory["result"]["roots"].as_array().unwrap().len(), 2);
	assert_eq!(
		inventory["result"]["projects"][0]["reference"],
		"standalone repo"
	);
}
