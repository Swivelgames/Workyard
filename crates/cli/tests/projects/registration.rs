use super::fixtures::{cli_result, inventory, invoke, references};
use crate::support::{assert_ok, git, init_workspace, remote_clone, repo, text, workyard};
use serde_json::json;
use std::{
	collections::BTreeSet,
	fs,
	path::{Path, PathBuf},
	sync::Barrier,
	thread,
};

#[test]
fn simultaneous_explicit_admissions_merge_registrations_and_untracking_preserves_membership() {
	let fixture = tempfile::tempdir().unwrap();
	let root = fixture.path().join("workspace");
	init_workspace(&root);
	let api = repo(fixture.path(), "api");
	let web = repo(fixture.path(), "web");

	let ready = Barrier::new(2);
	thread::scope(|scope| {
		for (task, project) in [("one", &api), ("two", &web)] {
			let ready = &ready;
			let root = &root;
			scope.spawn(move || {
				ready.wait();
				assert_ok(&workyard(
					root,
					&[
						"create",
						task,
						text(project),
						"--no-fetch",
						"--from",
						"main",
					],
				));
			});
		}
	});
	assert_eq!(references(&inventory(&root)), ["api", "web"]);

	let untrack = workyard(
		&root,
		&["projects", "untrack", text(&api), text(&web), "--json"],
	);
	assert_ok(&untrack);
	assert_eq!(cli_result(&untrack)["changed"].as_array().unwrap().len(), 2);
	assert!(references(&inventory(&root)).is_empty());
	assert!(root.join("tasks/one/api/.git").is_file());
	assert!(root.join("tasks/two/web/.git").is_file());
}

#[test]
fn failed_registry_write_preserves_admitted_work_and_tracking_can_retry_independently() {
	let fixture = tempfile::tempdir().unwrap();
	let workspace = fixture.path().join("workspace");
	init_workspace(&workspace);
	let api = repo(fixture.path(), "api");

	// Block the registration write while leaving task admission and Git usable.
	let lock = workspace.join(".workyard/locks/projects.lock");
	fs::create_dir(&lock).unwrap();
	let create = workyard(
		&workspace,
		&["create", "topic", text(&api), "--from", "main", "--json"],
	);
	assert!(!create.status.success());
	let result = cli_result(&create);
	assert_eq!(result["operation"]["outcome"], "succeeded");
	assert_eq!(result["lifecycle"], "active");
	assert!(
		result["registration_problem"]["remedy"]
			.as_str()
			.unwrap()
			.contains("workyard projects track")
	);
	assert!(workspace.join("tasks/topic/api/.git").is_file());
	assert!(references(&inventory(&workspace)).is_empty());

	fs::remove_dir(&lock).unwrap();
	let track = invoke(&workspace, &["projects", "track"], json!({"paths": [api]}));
	assert_eq!(track["result"]["changed"].as_array().unwrap().len(), 1);
	assert_eq!(references(&inventory(&workspace)), ["api"]);
}

#[test]
fn registrations_anchor_internal_paths_to_the_workspace_and_keep_external_roots_absolute() {
	let fixture = tempfile::tempdir().unwrap();
	let workspace = fixture.path().join("workspace");
	init_workspace(&workspace);
	let caller = workspace.join("nested");
	fs::create_dir(&caller).unwrap();

	let external = fixture.path().join("external");
	let track = workyard(
		&caller,
		&[
			"projects",
			"track",
			"future-clones",
			text(&external),
			"--json",
		],
	);
	assert_ok(&track);
	assert_eq!(cli_result(&track)["changed"].as_array().unwrap().len(), 2);
	assert!(!external.exists());
	assert!(!caller.join("future-clones").exists());

	let manifest = fs::read(workspace.join(".workyard/workspace.json")).unwrap();
	let manifest: serde_json::Value = serde_json::from_slice(&manifest).unwrap();
	let roots = manifest["project_roots"].as_array().unwrap();
	assert!(roots.contains(&json!(Path::new("nested").join("future-clones"))));
	let absolute = fixture.path().canonicalize().unwrap().join("external");
	assert!(roots.contains(&json!(absolute)));

	let repeat = invoke(
		&caller,
		&["projects", "track"],
		json!({"paths":["future-clones", external]}),
	);
	assert_eq!(repeat["result"]["changed"], json!([]));
	fs::create_dir(&external).unwrap();
	repo(&external, "web");
	assert_eq!(references(&inventory(&workspace)), ["web"]);
}

#[test]
fn two_explicit_linked_roots_remain_independent_after_one_participant_is_admitted() {
	let fixture = tempfile::tempdir().unwrap();
	let workspace = fixture.path().join("workspace");
	init_workspace(&workspace);
	let api = repo(fixture.path(), "api");
	let aliases = fixture.path().join("aliases");
	fs::create_dir(&aliases).unwrap();
	let linked = aliases.join("api");
	git(&api, &["worktree", "add", "-b", "alias", text(&linked)]);

	let create = workyard(
		&workspace,
		&[
			"create",
			"topic",
			text(&api),
			text(&linked),
			"--from",
			"main",
			"--json",
		],
	);
	assert_ok(&create);
	assert_eq!(
		cli_result(&create)["participants"]
			.as_array()
			.unwrap()
			.len(),
		1
	);
	let roots = registered_paths(&workspace);
	assert!(roots.contains(&api.canonicalize().unwrap()));
	assert!(roots.contains(&linked.canonicalize().unwrap()));

	assert_ok(&workyard(&workspace, &["projects", "untrack", text(&api)]));
	let listed = inventory(&workspace);
	assert_eq!(listed["projects"].as_array().unwrap().len(), 1);
	assert_eq!(
		listed["projects"][0]["path"],
		json!(linked.canonicalize().unwrap())
	);
}

#[test]
fn partial_admission_registers_only_explicit_participants_with_worktrees_until_recovery() {
	for action in ["create", "amend"] {
		let fixture = tempfile::tempdir().unwrap();
		let workspace = fixture.path().join("workspace");
		init_workspace(&workspace);
		let projects = workspace.join("projects");
		fs::create_dir(&projects).unwrap();
		repo(&projects, "tracked");
		let ready = repo(fixture.path(), "ready");
		let (deferred, remote) = remote_clone(fixture.path(), "deferred");
		let offline = fixture.path().join("offline.git");
		fs::rename(&remote, &offline).unwrap();

		if action == "amend" {
			assert_ok(&workyard(
				&workspace,
				&["create", "topic", "tracked", "--from", "main"],
			));
		}
		let partial = workyard(
			&workspace,
			&[
				action,
				"topic",
				"tracked",
				text(&ready),
				text(&deferred),
				"--from",
				"main",
			],
		);
		assert!(
			!partial.status.success(),
			"{action} should report the failed fetch"
		);
		let task = workspace.join("tasks/topic");
		assert!(task.join("ready/.git").is_file(), "{action}");
		assert!(!task.join("deferred/.git").exists(), "{action}");
		let default_root = projects.canonicalize().unwrap();
		let expected = BTreeSet::from([default_root.clone(), ready.canonicalize().unwrap()]);
		assert_eq!(registered_paths(&workspace), expected, "{action}");

		// Recovery admits the deferred project and respects the user's untracking.
		assert_ok(&workyard(
			&workspace,
			&["projects", "untrack", text(&ready)],
		));
		fs::rename(&offline, &remote).unwrap();
		assert_ok(&workyard(&workspace, &["recover", "topic"]));
		assert!(task.join("deferred/.git").is_file());
		let expected = BTreeSet::from([default_root, deferred.canonicalize().unwrap()]);
		assert_eq!(registered_paths(&workspace), expected, "{action} recovery");
	}
}

#[test]
fn explicit_subdirectory_admits_and_registers_the_enclosing_repository() {
	let fixture = tempfile::tempdir().unwrap();
	let workspace = fixture.path().join("workspace");
	init_workspace(&workspace);
	let api = repo(fixture.path(), "api");
	let subdirectory = api.join("src/module");
	fs::create_dir_all(&subdirectory).unwrap();

	assert_ok(&workyard(
		&workspace,
		&[
			"create",
			"topic",
			"--repo",
			text(&subdirectory),
			"--from",
			"main",
		],
	));
	assert!(workspace.join("tasks/topic/api/.git").is_file());
	let roots = registered_paths(&workspace);
	assert!(roots.contains(&api.canonicalize().unwrap()));
	assert!(!roots.contains(&subdirectory.canonicalize().unwrap()));
	assert_eq!(references(&inventory(&workspace)), ["api"]);
}

#[cfg(unix)]
#[test]
fn explicit_source_symlink_keeps_its_name_for_the_participant_and_future_selection() {
	let fixture = tempfile::tempdir().unwrap();
	let workspace = fixture.path().join("workspace");
	init_workspace(&workspace);
	let backing = repo(fixture.path(), "implementation");
	let alias = workspace.join("api");
	std::os::unix::fs::symlink(&backing, &alias).unwrap();

	assert_ok(&workyard(
		&workspace,
		&["create", "topic", "./api", "--from", "main"],
	));
	assert!(workspace.join("tasks/topic/api/.git").is_file());
	let roots = registered_paths(&workspace);
	assert!(roots.contains(&workspace.canonicalize().unwrap().join("api")));
	assert!(!roots.contains(&backing.canonicalize().unwrap()));
	assert_eq!(references(&inventory(&workspace)), ["api"]);
}

fn registered_paths(workspace: &Path) -> BTreeSet<PathBuf> {
	inventory(workspace)["roots"]
		.as_array()
		.unwrap()
		.iter()
		.map(|root| PathBuf::from(root["path"].as_str().unwrap()))
		.collect()
}

#[test]
fn untrack_accepts_the_displayed_root_left_behind_by_a_workspace_move() {
	let fixture = tempfile::tempdir().unwrap();
	let old = fixture.path().join("old");
	let new = fixture.path().join("new");
	init_workspace(&old);
	let api = repo(&old, "api");
	assert_ok(&workyard(&old, &["projects", "track", text(&api)]));
	fs::create_dir(&new).unwrap();
	for entry in ["tasks", ".workyard"] {
		fs::rename(old.join(entry), new.join(entry)).unwrap();
	}

	let displayed = api.canonicalize().unwrap();
	assert!(registered_paths(&new).contains(&displayed));
	let untrack = workyard(&new, &["projects", "untrack", text(&displayed), "--json"]);
	assert_ok(&untrack);
	assert_eq!(cli_result(&untrack)["changed"], json!([displayed]));
	assert!(!registered_paths(&new).contains(&displayed));
	assert!(displayed.is_dir());
}

#[test]
fn deferred_project_registration_follows_a_moved_workspace_without_rewriting_admission() {
	for action in ["create", "amend"] {
		let fixture = tempfile::tempdir().unwrap();
		let old = fixture.path().join("old");
		let new = fixture.path().join("new");
		init_workspace(&old);
		let projects = old.join("projects");
		fs::create_dir(&projects).unwrap();
		repo(&projects, "baseline");
		let ready = repo(fixture.path(), "ready");
		let (clone, remote) = remote_clone(fixture.path(), "deferred");
		let deferred = old.join("deferred");
		fs::rename(clone, &deferred).unwrap();
		let offline = fixture.path().join("offline.git");
		fs::rename(&remote, &offline).unwrap();

		if action == "amend" {
			assert_ok(&workyard(
				&old,
				&["create", "topic", "baseline", "--from", "main"],
			));
		}
		let partial = workyard(
			&old,
			&[
				action,
				"topic",
				text(&ready),
				text(&deferred),
				"--from",
				"main",
			],
		);
		assert!(!partial.status.success());
		assert!(old.join("tasks/topic/ready/.git").is_file());
		assert!(!old.join("tasks/topic/deferred/.git").exists());
		assert_eq!(references(&inventory(&old)), ["baseline", "ready"]);
		let admission =
			Path::new(".workyard/evidence").join(workyard_state::task_filename("topic"));
		let original_admission = fs::read(old.join(&admission)).unwrap();

		fs::rename(&old, &new).unwrap();
		fs::rename(&offline, &remote).unwrap();
		assert_ok(&workyard(&new, &["recover", "topic"]));
		assert!(new.join("tasks/topic/deferred/.git").is_file(), "{action}");
		let expected = BTreeSet::from([
			new.canonicalize().unwrap().join("projects"),
			new.join("deferred").canonicalize().unwrap(),
			ready.canonicalize().unwrap(),
		]);
		assert_eq!(registered_paths(&new), expected, "{action} recovery");
		assert_eq!(
			references(&inventory(&new)),
			["baseline", "deferred", "ready"]
		);
		assert_eq!(fs::read(new.join(&admission)).unwrap(), original_admission);

		assert_ok(&workyard(&new, &["recover", "topic"]));
		assert_eq!(registered_paths(&new), expected);
	}
}
