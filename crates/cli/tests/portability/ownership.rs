use super::*;
use workyard_core::{Control, RemovalGit};
use workyard_git::Runner;
use workyard_state::Reader;

fn rejected_incomplete_copy(workspace: &Path, api: &Path, active: &Path) {
	let status = workyard(workspace, &["status"]);
	assert!(String::from_utf8_lossy(&status.stdout).contains("needs attention"));
	for action in [&["recover"][..], &["remove", "active", "--force"][..]] {
		let result = workyard(workspace, action);
		assert!(!result.status.success(), "{action:?}");
		assert_eq!(registered(api, "active"), active.canonicalize().unwrap());
		assert_eq!(fs::read(active.join("file")).unwrap(), b"authored here");
	}
}

#[test]
fn incomplete_original_cannot_follow_an_activated_copy() {
	let fixture = tempfile::tempdir().unwrap();
	let old = fixture.path().join("original");
	let new = fixture.path().join("copy");
	let api = repo(fixture.path(), "api");
	init_workspace(&old);
	create(&old, "active", &api);
	copy_directory(&old, &new);
	assert_ok(&workyard(&new, &["recover", "--activate-here"]));
	let active = new.join("tasks/active/api");
	fs::write(active.join("file"), "authored here").unwrap();

	fs::rename(
		old.join("tasks/active/api"),
		fixture.path().join("archived"),
	)
	.unwrap();
	rejected_incomplete_copy(&old, &api, &active);
}

#[test]
fn copied_state_without_renamed_worktree_cannot_remove_the_original() {
	let fixture = tempfile::tempdir().unwrap();
	let old = fixture.path().join("original");
	let new = fixture.path().join("state-only");
	let api = repo(fixture.path(), "api");
	init_workspace(&old);
	create(&old, "active", &api);
	assert_ok(&workyard(
		&old,
		&[
			"amend",
			"active",
			"--repo",
			text(&api),
			"--repo-name",
			"backend",
		],
	));
	let active = old.join("tasks/active/backend");
	fs::write(active.join("file"), "authored here").unwrap();

	fs::create_dir_all(new.join("tasks")).unwrap();
	copy_directory(&old.join(".workyard"), &new.join(".workyard"));
	rejected_incomplete_copy(&new, &api, &active);
}

#[test]
fn forced_removal_cannot_act_on_files_replaced_after_inspection() {
	let fixture = tempfile::tempdir().unwrap();
	let workspace = fixture.path().join("workspace");
	let api = repo(fixture.path(), "api");
	init_workspace(&workspace);
	create(&workspace, "active", &api);
	let reader = Reader::open(&workspace.join(".workyard"), None).unwrap();
	let task = reader
		.tasks(Some("active"))
		.unwrap()
		.pop()
		.unwrap()
		.task
		.unwrap();
	let runner = Runner::default();
	let control = Control::new(|| false);
	let preview = smol::block_on(runner.inspect_removal(&task.participants[0], &control)).unwrap();

	let active = workspace.join("tasks/active/api");
	let detached = fixture.path().join("original-files");
	fs::rename(&active, &detached).unwrap();
	copy_directory(&detached, &active);
	fs::write(active.join("file"), "replacement edits").unwrap();
	assert!(smol::block_on(runner.remove_worktree(&preview, true, &control, || Ok(()))).is_err());
	assert_eq!(registered(&api, "active"), active.canonicalize().unwrap());
	assert_eq!(fs::read(active.join("file")).unwrap(), b"replacement edits");
}

#[test]
fn activation_hint_keeps_the_explicit_workspace_from_another_workspace() {
	let fixture = tempfile::tempdir().unwrap();
	let old = fixture.path().join("original");
	let new = fixture.path().join("another workspace");
	let api = repo(fixture.path(), "api");
	init_workspace(&old);
	create(&old, "active", &api);
	copy_directory(&old, &new);

	let supplied = Path::new("..").join("another workspace");
	let inspection = recover_rpc(&old, json!({"workspace": supplied}));
	let problem = &inspection["error"]["data"]["problem"];
	assert_eq!(problem["code"], "decision_required", "{inspection}");
	let remedy = problem["remedy"].as_str().unwrap();
	let target = format!("--workspace='{}'", supplied.display());
	assert!(remedy.contains(&target), "{remedy}");

	let accepted = workyard(
		&old,
		&["recover", "--activate-here", "--workspace", text(&supplied)],
	);
	assert_ok(&accepted);
	assert_eq!(
		registered(&api, "active"),
		new.join("tasks/active/api").canonicalize().unwrap()
	);
}

#[test]
fn displaced_absolute_records_cannot_remove_the_original_worktree() {
	let fixture = tempfile::tempdir().unwrap();
	let old = fixture.path().join("old");
	let new = fixture.path().join("partial");
	let api = repo(fixture.path(), "api");
	init_workspace(&old);
	create(&old, "active", &api);
	let original = old.join("tasks/active/api");
	fs::write(original.join("file"), "authored here").unwrap();
	fs::create_dir_all(new.join("tasks")).unwrap();
	let state = new.join(".workyard");
	copy_directory(&old.join(".workyard"), &state);

	for directory in ["tasks", "evidence"] {
		for entry in fs::read_dir(state.join(directory)).unwrap() {
			let path = entry.unwrap().path();
			let mut document: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
			let task = if path.extension().unwrap() == "effects" {
				&mut document["task"]
			} else {
				&mut document
			};
			task["tasks_root"] = json!(old.join("tasks"));
			let participant = &mut task["participants"][0];
			participant["destination"] = json!(&original);
			participant.as_object_mut().unwrap().remove("last_location");
			fs::write(path, serde_json::to_vec(&document).unwrap()).unwrap();
		}
	}
	rejected_incomplete_copy(&new, &api, &original);
}

#[test]
fn amend_cannot_unlink_a_selected_worktree_from_a_competing_copy() {
	let fixture = tempfile::tempdir().unwrap();
	let original = fixture.path().join("original");
	let copy = fixture.path().join("copy");
	let api = repo(fixture.path(), "api");
	let web = repo(fixture.path(), "web");
	init_workspace(&original);
	create(&original, "active", &api);
	assert_ok(&workyard(
		&original,
		&["amend", "active", "--repo", text(&web), "--from", "main"],
	));
	copy_directory(&original, &copy);
	for (workspace, content) in [(&original, "original draft"), (&copy, "copied draft")] {
		fs::write(workspace.join("tasks/active/web/file"), content).unwrap();
	}

	let rejected = workyard(&copy, &["amend", "active", "--remove", "web", "--force"]);
	assert!(!rejected.status.success());
	assert_eq!(
		registered(&web, "active"),
		original.join("tasks/active/web").canonicalize().unwrap()
	);
	assert_eq!(
		fs::read_to_string(original.join("tasks/active/web/file")).unwrap(),
		"original draft"
	);
	assert_eq!(
		fs::read_to_string(copy.join("tasks/active/web/file")).unwrap(),
		"copied draft"
	);
}

#[test]
fn final_unlink_refuses_new_bytes_in_the_same_worktree() {
	let fixture = tempfile::tempdir().unwrap();
	let workspace = fixture.path().join("workspace");
	let api = repo(fixture.path(), "api");
	init_workspace(&workspace);
	create(&workspace, "active", &api);
	let tree = workspace.join("tasks/active/api");
	fs::write(tree.join("file"), "disclosed edit").unwrap();
	let reader = Reader::open(&workspace.join(".workyard"), None).unwrap();
	let task = reader
		.tasks(Some("active"))
		.unwrap()
		.pop()
		.unwrap()
		.task
		.unwrap();
	let runner = Runner::default();
	let control = Control::default();
	let preview = smol::block_on(runner.inspect_removal(&task.participants[0], &control)).unwrap();

	fs::write(tree.join("file"), "changed after disclosure").unwrap();
	let refused = smol::block_on(runner.remove_worktree(&preview, true, &control, || {
		panic!("A stale observation must be refused before writer admission")
	}));
	assert!(
		matches!(refused, Err(workyard_core::UnlinkFailure::BeforeLaunch(problem))
		if problem.code == workyard_core::ProblemCode::DecisionRequired)
	);
	assert_eq!(
		fs::read_to_string(tree.join("file")).unwrap(),
		"changed after disclosure"
	);
	assert_eq!(registered(&api, "active"), tree.canonicalize().unwrap());
}
