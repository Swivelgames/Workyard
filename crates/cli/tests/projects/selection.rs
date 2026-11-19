use super::fixtures::{cli_result, complete, inventory, invoke, references};
use crate::support::{assert_ok, git, init_workspace, repo, text, workyard};
use serde_json::json;
use std::{fs, path::Path};

#[test]
fn default_root_populates_later_and_task_actions_share_project_selection() {
	let fixture = tempfile::tempdir().unwrap();
	let root = fixture.path();
	init_workspace(root);
	let projects = root.join("projects");
	let empty = inventory(root);
	assert!(references(&empty).is_empty());
	assert_eq!(empty["roots"][0]["status"], "missing");
	assert!(!projects.exists());

	fs::create_dir(&projects).unwrap();
	let api = repo(&projects, "api");
	let web = repo(&projects, "web");
	let populated = inventory(root);
	assert_eq!(references(&populated), ["api", "web"]);
	assert_eq!(
		invoke(root, &["projects", "list"], json!({}))["result"],
		populated
	);

	let create = workyard(
		root,
		&[
			"create",
			"fix/payments",
			"api",
			"--no-fetch",
			"--from",
			"main",
			"--json",
		],
	);
	assert_ok(&create);
	assert_eq!(cli_result(&create)["participants"][0]["name"], "api");

	let amend = invoke(
		root,
		&["amend"],
		json!({
			"task": "fix/payments",
			"projects": ["web"],
			"repositories": [{"path": "web", "name": "client"}],
			"from": "main",
			"no_fetch": true
		}),
	);
	assert_eq!(
		amend["result"]["changes"][0]["participant"], "client",
		"{amend}"
	);
	let task = root.join("tasks/fix/payments");
	assert!(task.join("api/.git").is_file());
	assert!(task.join("client/.git").is_file());
	assert!(!task.join("web").exists());

	git(&api, &["branch", "fix/existing"]);
	git(&web, &["branch", "fix/existing"]);
	assert_ok(&workyard(
		root,
		&["checkout", "fix/existing", "web", "--no-fetch", "api"],
	));
	for name in ["api", "web"] {
		let tree = root.join("tasks/fix/existing").join(name);
		assert_eq!(git(&tree, &["branch", "--show-current"]), "fix/existing");
	}
}

#[test]
fn qualified_choices_and_completions_match_before_any_task_or_registration_effects() {
	let fixture = tempfile::tempdir().unwrap();
	let root = fixture.path().join("workspace");
	init_workspace(&root);
	let projects = root.join("projects");
	for team in ["billing", "payments"] {
		fs::create_dir_all(projects.join(team)).unwrap();
		repo(&projects.join(team), "api");
	}
	let extra = repo(fixture.path(), "extra");
	let billing = Path::new("billing").join("api");
	let payments = Path::new("payments").join("api");
	let choices = [text(&billing), text(&payments)];
	let before = inventory(&root);

	let args = ["create", "topic", text(&extra), "api", "--json"];
	let ambiguous = workyard(&root, &args);
	assert!(!ambiguous.status.success());
	let result = cli_result(&ambiguous);
	assert_eq!(result["problem"]["code"], "project_ambiguous");
	assert_eq!(result["problem"]["candidates"], json!(choices));

	let rpc = invoke(
		&root,
		&["create"],
		json!({"task":"topic", "projects":["api"]}),
	);
	assert_eq!(
		rpc["error"]["data"]["problem"]["candidates"],
		json!(choices)
	);
	assert_eq!(complete(&root, &["create", "topic", "api"]), choices);
	assert_eq!(
		complete(&root, &["create", "topic", text(&payments), "api"]),
		[text(&billing)]
	);
	assert_eq!(inventory(&root), before);
	assert!(!root.join("tasks/topic").exists());
	assert_eq!(git(&extra, &["branch", "--list", "topic"]), "");

	let qualified = invoke(
		&root,
		&["create"],
		json!({
			"task":"topic", "projects": choices, "from":"main", "no_fetch":true
		}),
	);
	assert_eq!(
		qualified["result"]["operation"]["outcome"], "succeeded",
		"{qualified}"
	);
	for name in ["billing_api", "payments_api"] {
		assert!(root.join("tasks/topic").join(name).join(".git").is_file());
	}
}

#[test]
fn literal_name_collisions_allow_a_retry_with_an_override_and_amend_preserves_existing_names() {
	let fixture = tempfile::tempdir().unwrap();
	let root = fixture.path();
	init_workspace(root);
	let projects = root.join("projects");
	for team in ["billing", "payments"] {
		fs::create_dir_all(projects.join(team)).unwrap();
		repo(&projects.join(team), "api");
	}
	let literal = repo(&projects, "payments_api");

	let create = workyard(
		root,
		&[
			"create",
			"topic",
			"billing/api",
			"payments/api",
			"payments_api",
			"--from",
			"main",
			"--json",
		],
	);
	assert!(!create.status.success());
	let problem = &cli_result(&create)["problem"];
	assert_eq!(problem["code"], "participant_name_conflict");
	assert!(problem["remedy"].as_str().unwrap().contains("--repo-name"));
	assert!(!root.join("tasks/topic").exists());
	assert_eq!(git(&literal, &["branch", "--list", "topic"]), "");

	let suggested = &problem["suggested_override"];
	let path = suggested["path"].as_str().expect("the conflicting project");
	let name = suggested["name"]
		.as_str()
		.expect("an available participant name");
	let retry = workyard(
		root,
		&[
			"create",
			"topic",
			"billing/api",
			"payments/api",
			"payments_api",
			"--repo",
			path,
			"--repo-name",
			name,
			"--from",
			"main",
			"--json",
		],
	);
	assert_ok(&retry);
	let task = root.join("tasks/topic");
	assert!(task.join(name).join(".git").is_file());
	assert_eq!(
		cli_result(&retry)["participants"].as_array().unwrap().len(),
		3
	);

	assert_ok(&workyard(
		root,
		&["create", "stable", "billing/api", "--from", "main"],
	));
	assert_ok(&workyard(
		root,
		&["amend", "stable", "payments/api", "--from", "main"],
	));
	let stable = root.join("tasks/stable");
	let original = git(
		&stable.join("api"),
		&["rev-parse", "--path-format=absolute", "--git-common-dir"],
	);
	assert_eq!(
		Path::new(&original).canonicalize().unwrap(),
		projects.join("billing/api/.git").canonicalize().unwrap()
	);
	assert!(stable.join("payments_api/.git").is_file());
}

#[cfg(unix)]
#[test]
fn native_project_references_round_trip_from_cli_tracking_to_rpc_selection() {
	use std::{ffi::OsStr, os::unix::ffi::OsStrExt, process::Command};

	let fixture = tempfile::tempdir().unwrap();
	let workspace = fixture.path().join("workspace");
	init_workspace(&workspace);
	let original = repo(fixture.path(), "before");
	let name = OsStr::from_bytes(b"api-\xff");
	let repository = fixture.path().join(name);
	fs::rename(&original, &repository).unwrap();

	let track = Command::new(env!("CARGO_BIN_EXE_workyard"))
		.current_dir(&workspace)
		.args(["projects", "track"])
		.arg(&repository)
		.output()
		.unwrap();
	assert_ok(&track);

	let listed = inventory(&workspace);
	let reference = &listed["projects"][0]["reference"];
	let decoded: trine::NativePath = serde_json::from_value(reference.clone()).unwrap();
	assert_eq!(decoded.as_path(), Path::new(name));

	let create = invoke(
		&workspace,
		&["create"],
		json!({
			"task": "topic",
			"projects": [reference],
			"repositories": [{"path": reference, "name": "api"}],
			"from": "main",
			"no_fetch": true
		}),
	);
	assert_eq!(
		create["result"]["operation"]["outcome"], "succeeded",
		"{create}"
	);
	assert!(workspace.join("tasks/topic/api/.git").is_file());
}

#[cfg(unix)]
#[test]
fn create_and_amend_keep_the_lookup_identity_when_the_clone_changes_before_preflight() {
	use super::fixtures::swap_clone_on_inspection;

	for action in ["create", "amend"] {
		for refine_by_path in [false, true] {
			let fixture = tempfile::tempdir().unwrap();
			let workspace = fixture.path().join("workspace");
			init_workspace(&workspace);
			let projects = workspace.join("projects");
			fs::create_dir(&projects).unwrap();
			let api = repo(&projects, "api");
			let replacement = repo(fixture.path(), "replacement");
			if action == "amend" {
				repo(&projects, "original");
				assert_ok(&workyard(
					&workspace,
					&["create", "topic", "original", "--from", "main"],
				));
			}

			let mut command = swap_clone_on_inspection(&workspace, &api, &replacement);
			command.args([action, "topic", "api", "--json", "--from", "main"]);
			if refine_by_path {
				command.args(["--repo", text(&api), "--repo-name", "renamed"]);
			}
			let response = command.output().unwrap();
			assert!(!replacement.exists(), "fixture failed to replace the clone");
			let result = cli_result(&response);
			assert_eq!(
				result["problem"]["code"], "repository_changed",
				"{action} refined={refine_by_path}: {result}"
			);
			assert!(!response.status.success());
			assert_eq!(git(&api, &["branch", "--list", "topic"]), "");
			let task = workspace.join("tasks/topic");
			assert!(!task.join("api").exists());
			assert!(!task.join("renamed").exists());
			assert_eq!(task.join("original/.git").is_file(), action == "amend");
		}
	}
}
