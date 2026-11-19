use super::{create, state_bytes};
use crate::rpc::invoke;
use crate::support::{assert_ok, git, record, repo, text, workyard};
use serde_json::{Value, json};
use std::{fs, path::Path};

fn add(root: &Path, source: &Path) {
	assert_ok(&workyard(
		root,
		&[
			"amend",
			"feat/task",
			"--repo",
			text(source),
			"--from",
			"main",
		],
	));
}

fn participants(root: &Path) -> Vec<String> {
	let task = record(&root.join(".workyard"));
	task["participants"]
		.as_array()
		.unwrap()
		.iter()
		.map(|participant| participant["name"].as_str().unwrap().to_owned())
		.collect()
}

#[test]
fn removal_uses_membership_after_untracking_and_retains_branches_and_sibling_edits() {
	let fixture = tempfile::tempdir().unwrap();
	let root = fixture.path().join("workspace");
	let api = repo(fixture.path(), "api");
	let web = repo(fixture.path(), "web");
	let infra = repo(fixture.path(), "infra");
	create(&root, &api);
	add(&root, &web);
	add(&root, &infra);
	assert_ok(&workyard(&root, &["projects", "untrack", text(&infra)]));

	let tasks = root.join("tasks/feat/task");
	fs::write(tasks.join("api/authored"), "sibling edit").unwrap();
	git(
		&tasks.join("infra"),
		&["commit", "--allow-empty", "-m", "local work"],
	);
	let tip = git(&infra, &["rev-parse", "feat/task"]);
	let roots_before = workyard(&root, &["projects", "list", "--json"]);
	assert_ok(&roots_before);

	let removed = workyard(&root, &["amend", "feat/task", "--remove", "infra"]);
	assert_ok(&removed);
	let output = String::from_utf8(removed.stdout).unwrap();
	assert!(
		output.contains("feat/task") && output.contains("Removed: infra"),
		"{output}"
	);
	assert_eq!(participants(&root), ["api", "web"]);
	assert!(!tasks.join("infra").exists());
	assert!(!git(&infra, &["worktree", "list", "--porcelain"]).contains("refs/heads/feat/task"));
	assert_eq!(git(&infra, &["rev-parse", "feat/task"]), tip);
	assert_eq!(
		workyard(&root, &["projects", "list", "--json"]).stdout,
		roots_before.stdout
	);
	assert_eq!(
		fs::read_to_string(tasks.join("api/authored")).unwrap(),
		"sibling edit"
	);

	// A later amendment and publication repair must preserve the authorized omission.
	add(&root, &repo(fixture.path(), "worker"));
	let published = fs::read_dir(root.join(".workyard/tasks"))
		.unwrap()
		.next()
		.unwrap()
		.unwrap()
		.path();
	fs::remove_file(published).unwrap();
	assert_ok(&workyard(&root, &["recover", "feat/task"]));
	assert_eq!(participants(&root), ["api", "web", "worker"]);
	assert!(!tasks.join("infra").exists());
}

#[test]
fn last_unknown_and_conflicting_removals_are_read_only() {
	let fixture = tempfile::tempdir().unwrap();
	let root = fixture.path().join("workspace");
	let api = repo(fixture.path(), "api");
	create(&root, &api);
	let before = state_bytes(&root);
	for (extra, last) in [
		(vec!["--remove", "api"], true),
		(vec!["--remove", "absent"], false),
		(
			vec![
				"--remove",
				"api",
				"--repo",
				text(&api),
				"--repo-name",
				"other",
			],
			false,
		),
	] {
		let mut args = vec!["amend", "feat/task", "--force"];
		args.extend(extra);
		let rejected = workyard(&root, &args);
		assert!(!rejected.status.success(), "{args:?}");
		if last {
			let message = String::from_utf8(rejected.stdout).unwrap();
			assert!(
				message.contains("workyard remove") && message.contains("feat/task"),
				"{message}"
			);
		}
		assert_eq!(state_bytes(&root), before);
		assert!(root.join("tasks/feat/task/api/.git").is_file());
	}
}

#[test]
fn unattended_discard_requires_consent_and_reports_the_same_facts_over_json_and_rpc() {
	let fixture = tempfile::tempdir().unwrap();
	let root = fixture.path().join("workspace");
	create(&root, &repo(fixture.path(), "api"));
	for name in ["web", "infra"] {
		add(&root, &repo(fixture.path(), name));
	}
	let path = root.join("tasks/feat/task/web/authored");
	fs::write(&path, "local draft").unwrap();
	let before = state_bytes(&root);

	let args = [
		"amend",
		"feat/task",
		"--remove",
		"web",
		"--remove",
		"infra",
		"--yes",
		"--json",
	];
	let cli = workyard(&root, &args);
	assert!(!cli.status.success());
	let cli: Value = serde_json::from_slice(&cli.stdout).unwrap();
	let input = json!({"task":"feat/task","remove":["web","infra"],"yes":true});
	let rpc = invoke(&root, "amend", input.clone());
	let outcome = &rpc["error"]["data"];
	assert_eq!(cli["result"], *outcome, "{cli}");
	assert_eq!(outcome["problem"]["code"], "decision_required", "{rpc}");
	assert_eq!(outcome["decision"]["kind"], "discard_participants");
	assert_eq!(outcome["decision"]["task"], "feat/task");
	let selected = outcome["decision"]["participants"].as_array().unwrap();
	assert_eq!(selected.len(), 2);
	for name in ["web", "infra"] {
		assert!(selected.contains(&json!(name)));
	}
	let disclosures = outcome["disclosures"].as_array().unwrap();
	let web = disclosures
		.iter()
		.find(|d| d["participant"] == "web")
		.unwrap();
	assert_eq!(web["dirty_paths"], json!(["authored"]));
	assert_eq!(state_bytes(&root), before);
	assert_eq!(fs::read_to_string(path).unwrap(), "local draft");

	let mut authorized = input;
	authorized["force"] = true.into();
	let rpc = invoke(&root, "amend", authorized);
	assert_eq!(rpc["result"]["operation"]["outcome"], "succeeded", "{rpc}");
	let changes = rpc["result"]["changes"].as_array().unwrap();
	for name in ["web", "infra"] {
		assert!(changes.contains(&json!({"kind":"removed","participant":name})));
	}
	assert_eq!(participants(&root), ["api"]);
}

#[test]
fn force_preserves_an_unrelated_directory_at_the_selected_location() {
	let fixture = tempfile::tempdir().unwrap();
	let root = fixture.path().join("workspace");
	create(&root, &repo(fixture.path(), "api"));
	let web = repo(fixture.path(), "web");
	add(&root, &web);
	let tree = root.join("tasks/feat/task/web");
	let moved = fixture.path().join("moved-web");
	fs::rename(&tree, &moved).unwrap();
	fs::create_dir(&tree).unwrap();
	fs::write(tree.join("authored"), "unrelated work").unwrap();
	let before = state_bytes(&root);

	let rejected = workyard(&root, &["amend", "feat/task", "--remove", "web", "--force"]);
	assert!(!rejected.status.success());
	assert_eq!(state_bytes(&root), before);
	assert_eq!(
		fs::read_to_string(tree.join("authored")).unwrap(),
		"unrelated work"
	);
	assert!(moved.join(".git").is_file());
	assert!(git(&web, &["worktree", "list", "--porcelain"]).contains("refs/heads/feat/task"));
}

#[test]
fn last_member_guidance_names_the_workspace_selected_from_elsewhere() {
	let fixture = tempfile::tempdir().unwrap();
	let root = fixture.path().join("another workspace");
	create(&root, &repo(fixture.path(), "api"));
	let result = workyard(
		fixture.path(),
		&[
			"amend",
			"feat/task",
			"--workspace",
			"another workspace",
			"--remove",
			"api",
			"--json",
		],
	);
	assert!(!result.status.success());
	let response: Value = serde_json::from_slice(&result.stdout).unwrap();
	let problem = &response["result"]["problem"];
	assert_eq!(problem["code"], "empty_task");
	let command = problem["remedy"].as_str().unwrap();
	assert!(
		command.contains("workyard remove --workspace='another workspace' feat/task"),
		"{command}"
	);
}

#[path = "replacement.rs"]
mod replacement;
