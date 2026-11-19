//! Replacement keeps a usable worktree through failure and replay.
use super::*;

#[test]
fn same_name_replacement_of_only_member_survives_a_retry() {
	let fixture = tempfile::tempdir().unwrap();
	let root = fixture.path().join("workspace");
	let original = repo(fixture.path(), "api");
	let replacement = repo(fixture.path(), "replacement");
	create(&root, &original);
	let args = [
		"amend",
		"feat/task",
		"--remove",
		"api",
		"--repo",
		text(&replacement),
		"--repo-name",
		"api",
		"--from",
		"main",
		"--yes",
	];
	let result = workyard(&root, &args);
	assert_ok(&result);
	let summary = String::from_utf8(result.stdout).unwrap();
	assert!(summary.contains("Replaced: api"), "{summary}");
	assert!(!String::from_utf8_lossy(&result.stderr).contains(".workyard-incoming-"));

	let worktree = root.join("tasks/feat/task/api");
	assert_eq!(participants(&root), ["api"]);
	assert_eq!(
		git(
			&worktree,
			&["rev-parse", "--path-format=absolute", "--git-common-dir"]
		),
		git(
			&replacement,
			&["rev-parse", "--path-format=absolute", "--git-common-dir"]
		)
	);
	assert!(!git(&original, &["worktree", "list", "--porcelain"]).contains("refs/heads/feat/task"));
	fs::write(worktree.join("authored"), "replacement edit").unwrap();
	let before = state_bytes(&root);
	assert_ok(&workyard(&root, &args));
	assert_eq!(state_bytes(&root), before);
	assert_eq!(
		fs::read_to_string(worktree.join("authored")).unwrap(),
		"replacement edit"
	);
}

#[test]
fn failed_fetch_for_the_sole_replacement_leaves_the_incumbent_usable() {
	let fixture = tempfile::tempdir().unwrap();
	let root = fixture.path().join("workspace");
	let original = repo(fixture.path(), "api");
	let replacement = repo(fixture.path(), "replacement");
	create(&root, &original);
	let missing = fixture.path().join("offline.git");
	git(&replacement, &["remote", "add", "origin", text(&missing)]);

	let result = workyard(
		&root,
		&[
			"amend",
			"feat/task",
			"--remove",
			"api",
			"--repo",
			text(&replacement),
			"--repo-name",
			"api",
			"--from",
			"main",
			"--yes",
		],
	);
	assert!(!result.status.success());
	let tree = root.join("tasks/feat/task/api");
	assert!(tree.join(".git").is_file());
	assert_eq!(git(&tree, &["branch", "--show-current"]), "feat/task");
	assert_eq!(
		git(
			&tree,
			&["rev-parse", "--path-format=absolute", "--git-common-dir"]
		),
		git(
			&original,
			&["rev-parse", "--path-format=absolute", "--git-common-dir"]
		)
	);
}

#[test]
fn staging_and_case_aliases_respect_existing_entries_on_this_volume() {
	let fixture = tempfile::tempdir().unwrap();
	let root = fixture.path().join("workspace");
	let original = repo(fixture.path(), "api");
	let replacement = repo(fixture.path(), "new-api");
	create(&root, &original);
	let directory = root.join("tasks/feat/task");
	let unrelated = directory.join(".workyard-incoming-1");
	fs::write(&unrelated, "owned by user").unwrap();
	let destination = directory.join("API");
	let case_sensitive = !destination.exists();
	let args = [
		"amend",
		"feat/task",
		"--remove",
		"api",
		"--repo",
		text(&replacement),
		"--repo-name",
		"API",
		"--from",
		"main",
		"--yes",
	];
	if case_sensitive {
		fs::create_dir(&destination).unwrap();
		assert!(!workyard(&root, &args).status.success());
		assert!(directory.join("api/.git").is_file());
		fs::remove_dir(&destination).unwrap();
	}
	assert_ok(&workyard(&root, &args));
	assert!(destination.join(".git").is_file());
	assert_eq!(fs::read_to_string(unrelated).unwrap(), "owned by user");
}
