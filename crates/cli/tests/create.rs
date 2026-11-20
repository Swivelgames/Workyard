mod support;
use std::{fs, process::Command};
use support::*;

#[test]
fn creates_two_repositories_in_selected_workspace_and_keeps_sources_untouched() {
	let fixture = tempfile::tempdir().unwrap();
	let api = repo(fixture.path(), "api");
	let web = repo(fixture.path(), "web");
	let root = fixture.path().join("workspace");
	let state = root.join(".workyard");
	init_workspace(&root);
	let output = workyard(
		fixture.path(),
		&[
			"create",
			"feat/payments",
			"--repo",
			text(&api),
			"--repo",
			text(&web),
			"--from",
			"main",
			"--workspace",
			text(&root),
		],
	);
	assert_ok(&output);
	for source in [&api, &web] {
		assert_eq!(git(source, &["branch", "--show-current"]), "main");
	}
	for name in ["api", "web"] {
		assert_eq!(
			git(
				&root.join("tasks/feat/payments").join(name),
				&["branch", "--show-current"]
			),
			"feat/payments"
		);
	}
	let stored = record(&state);
	assert_eq!(stored["operation"]["outcome"], "succeeded");
	assert_eq!(stored["participants"].as_array().unwrap().len(), 2);
	assert!(root.join(".workyard").is_dir());
	assert_eq!(fs::read_dir(state.join("evidence")).unwrap().count(), 2);
}

#[test]
fn existing_paths_and_branch_conflicts_leave_other_repos_untouched() {
	let fixture = tempfile::tempdir().unwrap();
	let api = repo(fixture.path(), "api");
	let web = repo(fixture.path(), "web");
	git(&web, &["branch", "feat/existing"]);
	let root = fixture.path().join("workspace");
	init_workspace(&root);
	let output = workyard(
		&root,
		&[
			"create",
			"feat/existing",
			"--repo",
			text(&api),
			"--repo",
			text(&web),
			"--from",
			"main",
		],
	);
	assert!(!output.status.success());
	assert!(!root.join("tasks/feat/existing").exists());
	assert!(!git(&api, &["branch", "--list", "feat/existing"]).contains("feat/existing"));
	fs::create_dir_all(root.join("tasks/feat/occupied")).unwrap();
	let output = workyard(
		&root,
		&[
			"checkout",
			"feat/occupied",
			"--repo",
			text(&api),
			"--from",
			"main",
		],
	);
	assert!(!output.status.success());
	assert!(String::from_utf8_lossy(&output.stdout).contains("task_exists"));
}

#[test]
fn checkout_reuses_local_branch_without_resetting_its_history() {
	let fixture = tempfile::tempdir().unwrap();
	let api = repo(fixture.path(), "api");
	git(&api, &["checkout", "-b", "feat/reuse"]);
	fs::write(api.join("file"), "feature\n").unwrap();
	git(&api, &["commit", "-am", "feature"]);
	let tip = git(&api, &["rev-parse", "HEAD"]);
	git(&api, &["checkout", "main"]);
	let root = fixture.path().join("workspace");
	init_workspace(&root);
	assert_ok(&workyard(
		&root,
		&[
			"checkout",
			"feat/reuse",
			"--repo",
			text(&api),
			"--from",
			"main",
		],
	));
	assert_eq!(
		git(&root.join("tasks/feat/reuse/api"), &["rev-parse", "HEAD"]),
		tip
	);
}

#[test]
fn missing_default_requires_an_explicit_source() {
	let fixture = tempfile::tempdir().unwrap();
	let api = repo(fixture.path(), "api");
	let root = fixture.path().join("workspace");
	init_workspace(&root);
	let output = workyard(&root, &["create", "feat/new", "--repo", text(&api)]);
	assert!(!output.status.success());
	assert!(String::from_utf8_lossy(&output.stdout).contains("default_branch_unknown"));
}

#[test]
fn explicit_uninitialized_workspace_does_not_fall_back_or_bootstrap() {
	let fixture = tempfile::tempdir().unwrap();
	let api = repo(fixture.path(), "api");
	let root = fixture.path().join("workspace");
	let missing = fixture.path().join("missing");
	init_workspace(&root);
	assert_ok(&workyard(
		&root,
		&["create", "feat/one", "--repo", text(&api), "--from", "main"],
	));
	let output = workyard(
		&root,
		&[
			"create",
			"feat/two",
			"--repo",
			text(&api),
			"--from",
			"main",
			"--workspace",
			text(&missing),
		],
	);
	assert!(!output.status.success());
	assert!(!missing.exists());
	assert!(!root.join("tasks/feat/two").exists());
}

#[test]
fn remote_only_checkout_tracks_target_and_default_creation_uses_local_tip() {
	let fixture = tempfile::tempdir().unwrap();
	let (api, remote) = remote_clone(fixture.path(), "api");
	git(&remote, &["branch", "feat/remote"]);
	git(&api, &["fetch"]);
	let root = fixture.path().join("workspace");
	init_workspace(&root);
	let rejected = workyard(&root, &["create", "feat/remote", "--repo", text(&api)]);
	assert!(!rejected.status.success());
	assert!(!root.join("tasks/feat/remote").exists());
	assert_ok(&workyard(
		&root,
		&["checkout", "feat/remote", "--repo", text(&api)],
	));
	assert_eq!(
		git(
			&root.join("tasks/feat/remote/api"),
			&["rev-parse", "--abbrev-ref", "@{upstream}"]
		),
		"origin/feat/remote"
	);

	let old_tip = git(&api, &["rev-parse", "main"]);
	let upstream = fixture.path().join("api-upstream");
	fs::write(upstream.join("file"), "remote update\n").unwrap();
	git(&upstream, &["commit", "-am", "remote update"]);
	git(&upstream, &["push", text(&remote), "main"]);
	assert_ok(&workyard(
		&root,
		&["create", "feat/local", "--repo", text(&api)],
	));
	assert_eq!(
		git(&root.join("tasks/feat/local/api"), &["rev-parse", "HEAD"]),
		old_tip
	);
	assert_ne!(git(&api, &["rev-parse", "origin/main"]), old_tip);
	assert_ok(&workyard(
		&root,
		&[
			"create",
			"feat/fresh",
			"--repo",
			text(&api),
			"--from",
			"origin/main",
		],
	));
	assert_eq!(
		git(&root.join("tasks/feat/fresh/api"), &["rev-parse", "HEAD"]),
		git(&api, &["rev-parse", "origin/main"])
	);
}

#[test]
fn fetch_revalidation_stops_after_prior_participant_success() {
	let fixture = tempfile::tempdir().unwrap();
	let api = repo(fixture.path(), "api");
	let (web, remote) = remote_clone(fixture.path(), "web");
	// The target is deliberately absent from the clone's cached remote refs.
	git(&remote, &["branch", "feat/race"]);
	let root = fixture.path().join("workspace");
	init_workspace(&root);
	let output = workyard(
		&root,
		&[
			"create",
			"feat/race",
			"--repo",
			text(&api),
			"--repo",
			text(&web),
			"--from",
			"main",
		],
	);
	assert!(!output.status.success());
	assert!(root.join("tasks/feat/race/api/.git").exists());
	assert!(!root.join("tasks/feat/race/web").exists());
	let stored = record(&root.join(".workyard"));
	assert_eq!(stored["participants"][0]["phase"], "completed");
	assert_eq!(stored["participants"][1]["phase"], "fetching");
	assert_eq!(stored["problem"]["code"], "branch_exists");
}

#[test]
fn rpc_and_cli_create_equivalent_task_outcomes() {
	use std::io::Write;
	let fixture = tempfile::tempdir().unwrap();
	let api = repo(fixture.path(), "api");
	let root = fixture.path().join("workspace");
	init_workspace(&root);
	assert_ok(&workyard(
		&root,
		&["create", "feat/cli", "--repo", text(&api), "--from", "main"],
	));
	let mut child = Command::new(env!("CARGO_BIN_EXE_workyard"))
		.current_dir(&root)
		.arg("rpc")
		.stdin(std::process::Stdio::piped())
		.stdout(std::process::Stdio::piped())
		.stderr(std::process::Stdio::piped())
		.spawn()
		.unwrap();
	let request = serde_json::json!({"jsonrpc":"2.0","id":1,"method":"invoke","params":{
		"action":["create"],"input":{"task":"feat/rpc","repositories":[{"path":api}],"from":"main"},"observe":true
	}});
	writeln!(child.stdin.take().unwrap(), "{request}").unwrap();
	let output = child.wait_with_output().unwrap();
	assert_ok(&output);
	let lines: Vec<serde_json::Value> = String::from_utf8(output.stdout)
		.unwrap()
		.lines()
		.map(|line| serde_json::from_str(line).unwrap())
		.collect();
	assert!(lines.iter().any(|message| message["method"] == "progress"));
	let terminal = lines.iter().find(|message| message["id"] == 1).unwrap();
	assert_eq!(terminal["result"]["operation"]["outcome"], "succeeded");
	assert_eq!(terminal["result"]["participants"][0]["phase"], "completed");
	assert_eq!(
		git(&root.join("tasks/feat/cli/api"), &["rev-parse", "HEAD"]),
		git(&root.join("tasks/feat/rpc/api"), &["rev-parse", "HEAD"])
	);
}

#[test]
fn aliases_deduplicate_and_conflicting_names_fail_before_worktrees() {
	let fixture = tempfile::tempdir().unwrap();
	let api = repo(fixture.path(), "api");
	let root = fixture.path().join("workspace");
	init_workspace(&root);
	assert_ok(&workyard(
		&root,
		&[
			"create",
			"feat/alias",
			"--repo",
			text(&api),
			"--repo",
			text(&api),
			"--from",
			"main",
		],
	));
	assert_eq!(
		record(&root.join(".workyard"))["participants"]
			.as_array()
			.unwrap()
			.len(),
		1
	);
	let output = workyard(
		&root,
		&[
			"create",
			"feat/conflict",
			"--repo",
			text(&api),
			"--repo",
			text(&api),
			"--repo-name",
			"different",
			"--from",
			"main",
		],
	);
	assert!(!output.status.success());
	assert!(!root.join("tasks/feat/conflict").exists());
}

#[test]
fn explicit_remote_source_wins_over_same_spelled_local_branch() {
	let fixture = tempfile::tempdir().unwrap();
	let (api, remote) = remote_clone(fixture.path(), "api");
	git(&api, &["branch", "origin/main", "main"]);
	let upstream = fixture.path().join("api-upstream");
	fs::write(upstream.join("file"), "updated\n").unwrap();
	git(&upstream, &["commit", "-am", "update"]);
	git(&upstream, &["push", text(&remote), "main"]);
	let root = fixture.path().join("workspace");
	init_workspace(&root);
	assert_ok(&workyard(
		&root,
		&[
			"create",
			"feat/fresh",
			"--repo",
			text(&api),
			"--from",
			"origin/main",
		],
	));
	assert_eq!(
		git(&root.join("tasks/feat/fresh/api"), &["rev-parse", "HEAD"]),
		git(&api, &["rev-parse", "refs/remotes/origin/main"])
	);
	assert_ne!(
		git(&root.join("tasks/feat/fresh/api"), &["rev-parse", "HEAD"]),
		git(&api, &["rev-parse", "refs/heads/origin/main"])
	);
}

#[test]
fn repeating_create_reports_existing_task_without_mutating_it() {
	let fixture = tempfile::tempdir().unwrap();
	let api = repo(fixture.path(), "api");
	let root = fixture.path().join("workspace");
	init_workspace(&root);
	let args = [
		"create",
		"feat/repeat",
		"--repo",
		text(&api),
		"--from",
		"main",
	];
	assert_ok(&workyard(&root, &args));
	let before = record(&root.join(".workyard"));
	let output = workyard(&root, &args);
	assert!(!output.status.success());
	assert!(String::from_utf8_lossy(&output.stdout).contains("task_exists"));
	assert_eq!(record(&root.join(".workyard")), before);
}

#[cfg(unix)]
#[test]
fn native_non_unicode_repository_paths_round_trip_in_state() {
	use std::os::unix::ffi::OsStringExt;
	let fixture = tempfile::tempdir().unwrap();
	let original = repo(fixture.path(), "api");
	let native = fixture
		.path()
		.join(std::ffi::OsString::from_vec(b"repo-\xff".to_vec()));
	fs::rename(original, &native).unwrap();
	let root = fixture.path().join("workspace");
	init_workspace(&root);
	let output = Command::new(env!("CARGO_BIN_EXE_workyard"))
		.current_dir(&root)
		.args(["create", "feat/native", "--repo"])
		.arg(&native)
		.args(["--repo-name", "api", "--from", "main"])
		.output()
		.unwrap();
	assert_ok(&output);
	let stored = record(&root.join(".workyard"));
	assert!(stored["participants"][0]["source"]["unix"].is_array());
	assert!(root.join("tasks/feat/native/api/.git").exists());
}

#[cfg(unix)]
#[test]
fn real_git_success_survives_post_checkout_publication_failure() {
	use std::os::unix::fs::PermissionsExt;
	let fixture = tempfile::tempdir().unwrap();
	let api = repo(fixture.path(), "api");
	let web = repo(fixture.path(), "web");
	let root = fixture.path().join("workspace");
	init_workspace(&root);
	let hook = api.join(".git/hooks/post-checkout");
	fs::write(&hook, "#!/bin/sh\nchmod 500 \"$WORKYARD_FAULT_DIR\"\n").unwrap();
	fs::set_permissions(&hook, fs::Permissions::from_mode(0o700)).unwrap();
	let tasks = root.join(".workyard/tasks");
	let output = Command::new(env!("CARGO_BIN_EXE_workyard"))
		.current_dir(&root)
		.args([
			"create",
			"feat/publication",
			"--repo",
			text(&api),
			"--repo",
			text(&web),
			"--from",
			"main",
		])
		.env("WORKYARD_FAULT_DIR", &tasks)
		.output()
		.unwrap();
	fs::set_permissions(&tasks, fs::Permissions::from_mode(0o700)).unwrap();
	assert!(!output.status.success());
	let summary = String::from_utf8_lossy(&output.stdout);
	assert!(summary.contains("publication_uncertain"), "{summary}");
	assert!(summary.contains("creation recorded"), "{summary}");
	assert!(root.join("tasks/feat/publication/api/.git").exists());
	assert!(!root.join("tasks/feat/publication/web").exists());
	assert_eq!(
		record(&root.join(".workyard"))["participants"][0]["phase"],
		"creating"
	);
	let effects = root
		.join(".workyard/evidence")
		.join(workyard_state::task_filename("feat/publication"))
		.with_extension("effects");
	let evidence: serde_json::Value = serde_json::from_slice(&fs::read(effects).unwrap()).unwrap();
	assert_eq!(evidence["task"]["participants"][0]["phase"], "completed");
	assert!(evidence["pending"].is_null());
}
