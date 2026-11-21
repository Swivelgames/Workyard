use super::{create, state_bytes};
use crate::support::{assert_ok, git, record, repo, text, workyard};
use std::fs;

#[test]
fn relative_git_links_and_nonstandard_worktree_locations_are_preserved() {
	let fixture = tempfile::tempdir().unwrap();
	let api = repo(fixture.path(), "api");
	let root = fixture.path().join("workspace");
	create(&root, &api);
	let external = fixture.path().join("outside");
	git(
		&api,
		&[
			"worktree",
			"move",
			text(&root.join("tasks/feat/task/api")),
			text(&external),
		],
	);
	git(
		&api,
		&["worktree", "repair", "--relative-paths", text(&external)],
	);
	assert_ok(&workyard(
		&root,
		&["amend", "feat/task", "--repo", text(&api)],
	));
	assert_eq!(
		record(&root.join(".workyard"))["participants"][0]["destination"],
		serde_json::json!(external.canonicalize().unwrap())
	);
	let before = state_bytes(&root);
	assert_ok(&workyard(&root, &["status"]));
	assert_eq!(before, state_bytes(&root));
}

#[test]
fn task_contention_and_uncertain_writers_reject_even_noop_amend() {
	let fixture = tempfile::tempdir().unwrap();
	let api = repo(fixture.path(), "api");
	let root = fixture.path().join("workspace");
	create(&root, &api);
	let before = state_bytes(&root);
	{
		let store = workyard_state::Store::open_existing(&root.join(".workyard"), None).unwrap();
		let _guard = store.admit("feat/task").unwrap();
		let output = workyard(&root, &["amend", "feat/task", "--repo", text(&api)]);
		assert!(!output.status.success());
		assert!(String::from_utf8_lossy(&output.stdout).contains("resource_busy"));
	}
	assert_eq!(before, state_bytes(&root));
	assert_ok(&workyard(
		&root,
		&[
			"amend",
			"feat/task",
			"--repo",
			text(&api),
			"--repo-name",
			"legacy",
		],
	));
	let revision = fs::read_dir(root.join(".workyard/evidence"))
		.unwrap()
		.map(|entry| entry.unwrap().path())
		.find(|path| {
			path.extension()
				.is_some_and(|extension| extension == "revision")
		})
		.unwrap();
	let mut value: serde_json::Value =
		serde_json::from_slice(&fs::read(&revision).unwrap()).unwrap();
	value["current"]["pending"] = 0.into();
	fs::write(&revision, serde_json::to_vec(&value).unwrap()).unwrap();
	let before = state_bytes(&root);
	assert!(
		!workyard(&root, &["amend", "feat/task", "--repo", text(&api)])
			.status
			.success()
	);
	assert_eq!(before, state_bytes(&root));
}

#[cfg(unix)]
#[test]
fn native_repository_path_survives_amend_and_recovery() {
	use std::os::unix::ffi::OsStringExt;
	let fixture = tempfile::tempdir().unwrap();
	let api = repo(fixture.path(), "api");
	let web = repo(fixture.path(), "web");
	let native = fixture
		.path()
		.join(std::ffi::OsString::from_vec(b"web-\xff".to_vec()));
	fs::rename(web, &native).unwrap();
	let root = fixture.path().join("workspace");
	create(&root, &api);
	let output = std::process::Command::new(env!("CARGO_BIN_EXE_workyard"))
		.current_dir(&root)
		.args([
			"amend",
			"feat/task",
			"--no-fetch",
			"--from",
			"main",
			"--repo",
		])
		.arg(&native)
		.args(["--repo-name", "web"])
		.output()
		.unwrap();
	assert_ok(&output);
	assert_ok(&workyard(&root, &["recover", "feat/task"]));
	assert!(root.join("tasks/feat/task/web/.git").exists());
}

#[test]
fn amended_task_recovery_follows_an_external_repaired_backing_move() {
	let fixture = tempfile::tempdir().unwrap();
	let api = repo(fixture.path(), "api");
	let root = fixture.path().join("workspace");
	create(&root, &api);
	assert_ok(&workyard(
		&root,
		&[
			"amend",
			"feat/task",
			"--repo",
			text(&api),
			"--repo-name",
			"legacy",
		],
	));
	let moved = fixture.path().join("moved-api");
	fs::rename(api, &moved).unwrap();
	git(&moved, &["worktree", "repair"]);
	assert_ok(&workyard(&root, &["recover", "feat/task"]));
	assert_eq!(
		record(&root.join(".workyard"))["participants"][0]["common_dir"],
		serde_json::json!(moved.join(".git").canonicalize().unwrap())
	);
}
