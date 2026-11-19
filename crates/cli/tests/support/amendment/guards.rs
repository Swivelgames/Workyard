//! A selected worktree can contain local data that ordinary Git status hides.
use super::{create, state_bytes};
use crate::support::{assert_ok, git, repo, text, workyard};
use std::fs;

#[test]
fn hidden_index_entries_require_discard_consent_even_when_git_status_is_clean() {
	for flag in ["--assume-unchanged", "--skip-worktree"] {
		let fixture = tempfile::tempdir().unwrap();
		let root = fixture.path().join("workspace");
		create(&root, &repo(fixture.path(), "api"));
		let web = repo(fixture.path(), "web");
		assert_ok(&workyard(
			&root,
			&["amend", "feat/task", "--repo", text(&web), "--from", "main"],
		));
		let tree = root.join("tasks/feat/task/web");
		git(&tree, &["update-index", flag, "file"]);
		fs::write(tree.join("file"), "hidden local edit").unwrap();
		assert_eq!(git(&tree, &["status", "--porcelain"]), "");
		let before = state_bytes(&root);

		let refused = workyard(
			&root,
			&["amend", "feat/task", "--remove", "web", "--yes", "--json"],
		);
		assert!(!refused.status.success());
		let response: serde_json::Value = serde_json::from_slice(&refused.stdout).unwrap();
		assert_eq!(
			response["result"]["decision"]["kind"], "discard_participants",
			"{response}"
		);
		assert_eq!(
			response["result"]["disclosures"][0]["dirty_paths"],
			serde_json::json!(["file"])
		);
		assert_eq!(
			fs::read_to_string(tree.join("file")).unwrap(),
			"hidden local edit"
		);
		assert_eq!(state_bytes(&root), before);
	}
}

#[test]
fn a_known_writer_in_the_last_repository_blocks_the_entire_group_before_unlink() {
	let fixture = tempfile::tempdir().unwrap();
	let root = fixture.path().join("workspace");
	create(&root, &repo(fixture.path(), "api"));
	for name in ["web", "infra"] {
		let source = repo(fixture.path(), name);
		assert_ok(&workyard(
			&root,
			&[
				"amend",
				"feat/task",
				"--repo",
				text(&source),
				"--from",
				"main",
			],
		));
	}
	let locked = fixture.path().join("infra/.git/refs/heads/busy.lock");
	fs::write(&locked, "other Git writer").unwrap();
	let before = state_bytes(&root);

	let refused = workyard(
		&root,
		&[
			"amend",
			"feat/task",
			"--remove",
			"web",
			"--remove",
			"infra",
			"--force",
		],
	);
	assert!(!refused.status.success());
	assert_eq!(state_bytes(&root), before);
	assert!(root.join("tasks/feat/task/web/.git").is_file());
	assert!(root.join("tasks/feat/task/infra/.git").is_file());
	assert_eq!(fs::read_to_string(locked).unwrap(), "other Git writer");
}

#[test]
fn force_preserves_nested_git_repositories_in_a_selected_worktree() {
	let fixture = tempfile::tempdir().unwrap();
	let root = fixture.path().join("workspace");
	create(&root, &repo(fixture.path(), "api"));
	let web = repo(fixture.path(), "web");
	assert_ok(&workyard(
		&root,
		&["amend", "feat/task", "--repo", text(&web), "--from", "main"],
	));
	let tree = root.join("tasks/feat/task/web");
	for clone_args in [vec!["clone", "--bare"], vec!["clone"]] {
		let mut args = clone_args;
		args.extend([text(&web), "nested"]);
		git(&tree, &args);
		let nested = tree.join("nested");
		let head = git(&nested, &["rev-parse", "HEAD"]);
		let before = state_bytes(&root);
		let refused = workyard(&root, &["amend", "feat/task", "--remove", "web", "--force"]);
		assert!(!refused.status.success());
		assert_eq!(state_bytes(&root), before);
		assert_eq!(git(&nested, &["rev-parse", "HEAD"]), head);
		fs::remove_dir_all(nested).unwrap();
	}
}

#[test]
fn force_cannot_remove_a_worktree_containing_a_moved_sibling() {
	let fixture = tempfile::tempdir().unwrap();
	let root = fixture.path().join("workspace");
	let api = repo(fixture.path(), "api");
	let web = repo(fixture.path(), "web");
	create(&root, &api);
	assert_ok(&workyard(
		&root,
		&["amend", "feat/task", "--repo", text(&web), "--from", "main"],
	));
	let directory = root.join("tasks/feat/task");
	let nested = directory.join("api/embedded");
	git(
		&web,
		&[
			"worktree",
			"move",
			text(&directory.join("web")),
			text(&nested),
		],
	);
	let before = state_bytes(&root);

	let result = workyard(&root, &["amend", "feat/task", "--remove", "api", "--force"]);
	assert!(!result.status.success());
	assert_eq!(state_bytes(&root), before);
	assert!(nested.join(".git").is_file());
	assert!(directory.join("api/.git").is_file());
}
