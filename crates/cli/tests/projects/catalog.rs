use super::fixtures::{complete, inventory, invoke, references, snapshot};
use crate::support::{assert_ok, git, init_workspace, repo, text, workyard};
use serde_json::json;
use std::fs;

#[test]
fn scans_follow_git_identity_stop_at_repo_boundaries_and_leave_state_untouched() {
	let fixture = tempfile::tempdir().unwrap();
	let workspace = fixture.path().join("workspace");
	init_workspace(&workspace);
	let state = workspace.join(".workyard");
	let projects = workspace.join("projects");
	fs::create_dir(&projects).unwrap();

	let upstream = repo(fixture.path(), "upstream");
	for name in ["api", "another"] {
		git(&projects, &["clone", text(&upstream), name]);
	}
	git(
		&projects,
		&["clone", "--bare", text(&upstream), "archive.git"],
	);
	let api = projects.join("api");
	repo(&api, "inside-api");
	repo(&workspace.join("tasks"), "inside-tasks");
	repo(&state, "inside-state");

	let linked = fixture.path().join("linked");
	git(&api, &["worktree", "add", "-b", "linked", text(&linked)]);
	assert_ok(&workyard(
		&workspace,
		&["projects", "track", ".", text(&linked), text(&api)],
	));
	let before = snapshot(&state);
	let listed = inventory(&workspace);
	assert_eq!(references(&listed), ["another", "api", "archive.git"]);
	assert_eq!(
		complete(&workspace, &["create", "topic", "arc"]),
		["archive.git"]
	);
	assert_eq!(before, snapshot(&state));

	assert_ok(&workyard(
		&workspace,
		&["create", "topic", "api", "--no-fetch", "--from", "main"],
	));
	let untrack = invoke(
		&workspace,
		&["projects", "untrack"],
		json!({
			"paths": [".", "projects", api]
		}),
	);
	assert_eq!(untrack["result"]["changed"].as_array().unwrap().len(), 3);
	assert_eq!(references(&inventory(&workspace)), ["linked"]);
	assert!(workspace.join("tasks/topic/api/.git").is_file());
	assert_eq!(
		git(&api, &["rev-parse", "--verify", "refs/heads/topic"]),
		git(&workspace.join("tasks/topic/api"), &["rev-parse", "HEAD"])
	);
}

#[test]
fn incomplete_roots_report_the_problem_alongside_available_projects() {
	let fixture = tempfile::tempdir().unwrap();
	let root = fixture.path();
	init_workspace(root);
	let projects = root.join("projects");
	let broken = projects.join("broken");
	fs::create_dir_all(&broken).unwrap();
	fs::write(broken.join(".git"), b"gitdir: absent\n").unwrap();
	repo(&projects, "api");

	let before = snapshot(&root.join(".workyard"));
	let listed = inventory(root);
	assert_eq!(references(&listed), ["api"]);
	let coverage = &listed["roots"][0];
	assert_eq!(coverage["status"], "incomplete");
	assert!(!coverage["issues"].as_array().unwrap().is_empty());
	assert_eq!(snapshot(&root.join(".workyard")), before);
}

#[test]
fn qualified_paths_and_linked_aliases_complete_even_with_shorter_display_names() {
	let fixture = tempfile::tempdir().unwrap();
	let workspace = fixture.path();
	init_workspace(workspace);
	let projects = workspace.join("projects/billing/projects");
	fs::create_dir_all(&projects).unwrap();
	let api = repo(&projects, "api");
	let alternate = workspace.join("projects/secondary-api");
	git(&api, &["worktree", "add", "-b", "alt", text(&alternate)]);
	let listed = inventory(workspace);
	assert_eq!(references(&listed), ["api"]);

	let qualified = std::path::Path::new("billing").join("projects");
	let prefix = qualified.join("ap");
	let expected = qualified.join("api");
	for (prefix, expected) in [
		(text(&prefix), text(&expected)),
		("secondary-a", "secondary-api"),
	] {
		let positional = complete(workspace, &["create", "topic", prefix]);
		assert!(
			positional.iter().any(|value| value == expected),
			"{prefix}: {positional:?}"
		);
		let grouped = complete(workspace, &["create", "topic", "--repo", prefix]);
		assert!(
			grouped.iter().any(|value| value == expected),
			"{prefix}: {grouped:?}"
		);
	}
}
