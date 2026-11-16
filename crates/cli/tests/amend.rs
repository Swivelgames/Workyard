mod support;
use std::{fs, path::Path};
use support::{assert_ok, git, record, repo, text, workyard};

fn create(root: &Path, source: &Path) {
	fs::create_dir(root).unwrap();
	assert_ok(&workyard(
		root,
		&[
			"create",
			"--no-fetch",
			"--from",
			"main",
			"--repo",
			text(source),
			"feat/task",
		],
	));
}

fn state_bytes(root: &Path) -> Vec<(std::path::PathBuf, Vec<u8>)> {
	fn collect(path: &Path, files: &mut Vec<(std::path::PathBuf, Vec<u8>)>) {
		for entry in fs::read_dir(path).unwrap() {
			let path = entry.unwrap().path();
			if path.is_dir() {
				collect(&path, files);
			} else if path.extension().is_none_or(|extension| extension != "lock") {
				files.push((path.clone(), fs::read(path).unwrap()));
			}
		}
	}
	let mut files = Vec::new();
	collect(&root.join(".workyard"), &mut files);
	files.sort_by(|a, b| a.0.cmp(&b.0));
	files
}

#[test]
fn adds_then_repeats_without_changing_membership_or_options() {
	let fixture = tempfile::tempdir().unwrap();
	let api = repo(fixture.path(), "api");
	let web = repo(fixture.path(), "web");
	let root = fixture.path().join("workspace");
	create(&root, &api);
	assert_ok(&workyard(
		&root,
		&[
			"amend",
			"feat/task",
			"--no-fetch",
			"--from",
			"main",
			"--repo",
			text(&web),
		],
	));
	assert!(root.join("feat/task/web/.git").is_file());
	assert_eq!(
		record(&root.join(".workyard"))["participants"]
			.as_array()
			.unwrap()
			.len(),
		2
	);
	let before = state_bytes(&root);
	let result = workyard(
		&root,
		&[
			"amend",
			"feat/task",
			"--pull",
			"--checkout",
			"--repo",
			text(&web),
		],
	);
	assert_ok(&result);
	assert!(String::from_utf8_lossy(&result.stdout).contains("No changes needed"));
	assert_eq!(before, state_bytes(&root));
	assert_ok(&workyard(&root, &["status"]));
	assert_ok(&workyard(&root, &["remove", "--yes", "feat/task"]));
}

#[test]
fn renames_dirty_detached_participant_and_reuses_freed_name() {
	let fixture = tempfile::tempdir().unwrap();
	let api = repo(fixture.path(), "api");
	let replacement = repo(fixture.path(), "replacement");
	let root = fixture.path().join("workspace");
	create(&root, &api);
	git(&root.join("feat/task/api"), &["checkout", "--detach"]);
	fs::write(root.join("feat/task/api/authored"), "keep me").unwrap();
	assert_ok(&workyard(
		&root,
		&[
			"amend",
			"feat/task",
			"--no-fetch",
			"--from",
			"main",
			"--repo",
			text(&api),
			"--repo-name",
			"legacy-api",
			"--repo",
			text(&replacement),
			"--repo-name",
			"api",
		],
	));
	assert_eq!(
		fs::read_to_string(root.join("feat/task/legacy-api/authored")).unwrap(),
		"keep me"
	);
	assert_eq!(
		git(
			&root.join("feat/task/legacy-api"),
			&["rev-parse", "--abbrev-ref", "HEAD"]
		),
		"HEAD"
	);
	assert!(root.join("feat/task/api/.git").is_file());
	assert_ok(&workyard(
		&root,
		&[
			"amend",
			"feat/task",
			"--no-fetch",
			"--from",
			"main",
			"--repo",
			text(&api),
			"--repo-name",
			"legacy-api",
			"--repo",
			text(&replacement),
			"--repo-name",
			"api",
		],
	));
}

#[test]
fn supplied_moved_clone_repairs_links_without_manual_git_commands() {
	let fixture = tempfile::tempdir().unwrap();
	let api = repo(fixture.path(), "api");
	let root = fixture.path().join("workspace");
	create(&root, &api);
	let moved = fixture.path().join("moved-api");
	fs::rename(&api, &moved).unwrap();
	assert_ok(&workyard(
		&root,
		&[
			"amend",
			"feat/task",
			"--from",
			"main",
			"--repo",
			text(&moved),
		],
	));
	assert_eq!(
		git(
			&root.join("feat/task/api"),
			&["rev-parse", "--path-format=absolute", "--git-common-dir"]
		),
		text(&moved.join(".git"))
	);
	assert_eq!(
		record(&root.join(".workyard"))["participants"][0]["name"],
		"api"
	);
	assert_ok(&workyard(&root, &["status"]));
}

#[test]
fn whole_request_failure_preserves_records_and_dirty_unrelated_tree() {
	let fixture = tempfile::tempdir().unwrap();
	let api = repo(fixture.path(), "api");
	let web = repo(fixture.path(), "web");
	let root = fixture.path().join("workspace");
	create(&root, &api);
	fs::write(root.join("feat/task/api/authored"), "keep").unwrap();
	git(&web, &["branch", "feat/task"]);
	let before = state_bytes(&root);
	let failure = workyard(
		&root,
		&[
			"amend",
			"feat/task",
			"--from",
			"main",
			"--repo",
			text(&api),
			"--repo-name",
			"legacy",
			"--repo",
			text(&web),
		],
	);
	assert!(!failure.status.success());
	assert_eq!(before, state_bytes(&root));
	assert!(root.join("feat/task/api/authored").is_file());
	assert_ok(&workyard(
		&root,
		&[
			"amend",
			"feat/task",
			"--checkout",
			"--no-fetch",
			"--from",
			"main",
			"--repo",
			text(&web),
		],
	));
}

#[test]
fn retired_tasks_reject_amend_and_name_reuse_archives_revision() {
	let fixture = tempfile::tempdir().unwrap();
	let api = repo(fixture.path(), "api");
	let web = repo(fixture.path(), "web");
	let root = fixture.path().join("workspace");
	create(&root, &api);
	assert_ok(&workyard(
		&root,
		&["amend", "feat/task", "--from", "main", "--repo", text(&web)],
	));
	assert_ok(&workyard(&root, &["remove", "--yes", "feat/task"]));
	let before = state_bytes(&root);
	assert!(
		!workyard(
			&root,
			&["amend", "feat/task", "--from", "main", "--repo", text(&api)]
		)
		.status
		.success()
	);
	assert_eq!(before, state_bytes(&root));
	assert_ok(&workyard(
		&root,
		&[
			"checkout",
			"--no-fetch",
			"--from",
			"main",
			"--repo",
			text(&api),
			"feat/task",
		],
	));
	assert_eq!(record(&root.join(".workyard"))["generation"], 2);
	assert_eq!(
		record(&root.join(".workyard"))["participants"]
			.as_array()
			.unwrap()
			.len(),
		1
	);
	assert_ok(&workyard(&root, &["status"]));
}

#[test]
fn remote_addition_fetches_tracks_and_pulls_with_its_own_options() {
	let fixture = tempfile::tempdir().unwrap();
	let api = repo(fixture.path(), "api");
	let (web, remote) = support::remote_clone(fixture.path(), "web");
	git(&remote, &["branch", "feat/task", "main"]);
	let root = fixture.path().join("workspace");
	create(&root, &api);
	assert_ok(&workyard(
		&root,
		&[
			"amend",
			"feat/task",
			"--checkout",
			"--pull",
			"--repo",
			text(&web),
		],
	));
	let saved = record(&root.join(".workyard"));
	assert_eq!(saved["no_fetch"], true);
	assert_eq!(saved["pull"], false);
	assert_eq!(saved["participants"][1]["creation"]["pull"], true);
	assert_eq!(
		git(
			&root.join("feat/task/web"),
			&["rev-parse", "--abbrev-ref", "@{upstream}"]
		),
		"origin/feat/task"
	);
	let before = state_bytes(&root);
	assert_ok(&workyard(&root, &["recover", "feat/task"]));
	assert_eq!(before, state_bytes(&root));
}

#[test]
fn missing_published_record_is_rebuilt_from_revision_without_recreating_trees() {
	let fixture = tempfile::tempdir().unwrap();
	let api = repo(fixture.path(), "api");
	let web = repo(fixture.path(), "web");
	let root = fixture.path().join("workspace");
	create(&root, &api);
	assert_ok(&workyard(
		&root,
		&["amend", "feat/task", "--from", "main", "--repo", text(&web)],
	));
	let path = fs::read_dir(root.join(".workyard/tasks"))
		.unwrap()
		.next()
		.unwrap()
		.unwrap()
		.path();
	fs::remove_file(path).unwrap();
	fs::write(root.join("feat/task/web/authored"), "preserved").unwrap();
	assert_ok(&workyard(&root, &["recover", "feat/task"]));
	assert_eq!(
		record(&root.join(".workyard"))["participants"]
			.as_array()
			.unwrap()
			.len(),
		2
	);
	assert_eq!(
		fs::read_to_string(root.join("feat/task/web/authored")).unwrap(),
		"preserved"
	);
}

#[test]
fn rpc_uses_same_amend_contract() {
	use std::io::Write;
	let fixture = tempfile::tempdir().unwrap();
	let api = repo(fixture.path(), "api");
	let web = repo(fixture.path(), "web");
	let root = fixture.path().join("workspace");
	create(&root, &api);
	let mut child = std::process::Command::new(env!("CARGO_BIN_EXE_workyard"))
		.current_dir(&root)
		.arg("rpc")
		.stdin(std::process::Stdio::piped())
		.stdout(std::process::Stdio::piped())
		.spawn()
		.unwrap();
	let request = serde_json::json!({
		"jsonrpc": "2.0",
		"id": "amend-1",
		"method": "invoke",
		"params": {
			"action": ["amend"],
			"input": {
				"task": "feat/task",
				"no_fetch": true,
				"from": "main",
				"repositories": [{"path": web}]
			}
		}
	});
	writeln!(child.stdin.take().unwrap(), "{request}").unwrap();
	let output = child.wait_with_output().unwrap();
	let response: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
	assert_eq!(
		response["result"]["operation"]["kind"], "amend",
		"{response}"
	);
	assert_eq!(
		response["result"]["operation"]["outcome"], "succeeded",
		"{response}"
	);
	assert_eq!(
		response["result"]["changes"][0]["kind"], "added",
		"{response}"
	);
}

#[path = "support/amendment/identity.rs"]
mod identity;
#[path = "support/amendment/removal.rs"]
mod removal;
#[path = "support/amendment/retry.rs"]
mod retry;
