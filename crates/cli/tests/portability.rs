#[path = "portability/repair_scope.rs"]
mod repair_scope;
#[allow(dead_code)]
mod support;

use serde_json::{Value, json};
use std::{
	fs,
	io::Write,
	path::{Path, PathBuf},
	process::{Command, Stdio},
};
use support::{assert_ok, git, init_workspace, repo, text, workyard};

fn create(workspace: &Path, task: &str, project: &Path) {
	assert_ok(&workyard(
		workspace,
		&[
			"create",
			task,
			"--repo",
			text(project),
			"--no-fetch",
			"--from",
			"main",
		],
	));
}

fn copy_directory(from: &Path, to: &Path) {
	fs::create_dir(to).unwrap();
	for entry in fs::read_dir(from).unwrap() {
		let entry = entry.unwrap();
		let dest = to.join(entry.file_name());
		if entry.file_type().unwrap().is_dir() {
			copy_directory(&entry.path(), &dest);
		} else {
			fs::copy(entry.path(), dest).unwrap();
		}
	}
}

fn recover_rpc(workspace: &Path, input: Value) -> Value {
	let mut host = Command::new(env!("CARGO_BIN_EXE_workyard"))
		.arg("rpc")
		.current_dir(workspace)
		.stdin(Stdio::piped())
		.stdout(Stdio::piped())
		.spawn()
		.unwrap();
	let request = json!({"jsonrpc":"2.0","id":1,"method":"invoke","params":{"action":["recover"],"input":input}});
	writeln!(host.stdin.take().unwrap(), "{request}").unwrap();
	let output = host.wait_with_output().unwrap();
	assert_ok(&output);
	serde_json::from_slice(&output.stdout).unwrap()
}

fn registered(project: &Path, task: &str) -> PathBuf {
	let listing = git(project, &["worktree", "list", "--porcelain"]);
	let stanza = listing
		.split("\n\n")
		.find(|lines| {
			lines
				.lines()
				.any(|line| line == format!("branch refs/heads/{task}"))
		})
		.unwrap();
	PathBuf::from(
		stanza
			.lines()
			.next()
			.unwrap()
			.strip_prefix("worktree ")
			.unwrap(),
	)
	.canonicalize()
	.unwrap()
}

#[test]
fn one_recovery_can_activate_dirty_copies_across_tasks() {
	let fixture = tempfile::tempdir().unwrap();
	let old = fixture.path().join("workspace");
	let new = fixture.path().join("copy");
	let api = repo(fixture.path(), "api");
	init_workspace(&old);
	let tasks = ["feat/payments", "fix/login"];
	for task in tasks {
		create(&old, task, &api);
		fs::write(old.join("tasks").join(task).join("api/file"), "old edits").unwrap();
	}
	copy_directory(&old, &new);
	let before = git(&api, &["worktree", "list", "--porcelain"]);
	for task in tasks {
		fs::write(new.join("tasks").join(task).join("api/file"), "new edits").unwrap();
	}

	let inspect = recover_rpc(&new, json!({}));
	let decision = &inspect["error"]["data"];
	assert_eq!(decision["problem"]["code"], "decision_required");
	let rows = decision["participants"].as_array().unwrap();
	assert_eq!(rows.len(), 2);
	assert!(
		rows.iter()
			.all(|p| p["state"] == "choice_required" && p["registered"] != p["destination"])
	);
	assert_eq!(before, git(&api, &["worktree", "list", "--porcelain"]));
	assert!(
		!workyard(&new, &["remove", tasks[0], "--force"])
			.status
			.success()
	);

	let applied = recover_rpc(&new, json!({"activate_here":true}));
	assert_eq!(applied["result"]["nothing_to_recover"], false, "{applied}");
	for task in tasks {
		let relative = Path::new("tasks").join(task).join("api");
		assert_eq!(
			registered(&api, task),
			new.join(&relative).canonicalize().unwrap()
		);
		assert_eq!(
			fs::read(old.join(&relative).join("file")).unwrap(),
			b"old edits"
		);
		assert_eq!(
			fs::read(new.join(&relative).join("file")).unwrap(),
			b"new edits"
		);
	}

	let repeated = workyard(&new, &["recover", "--json"]);
	assert_ok(&repeated);
	let response: Value = serde_json::from_slice(&repeated.stdout).unwrap();
	assert_eq!(response["result"]["nothing_to_recover"], true);
	assert!(
		!workyard(&old, &["remove", tasks[1], "--force"])
			.status
			.success()
	);
}

#[test]
fn moving_twice_retains_external_backing_and_history() {
	let fixture = tempfile::tempdir().unwrap();
	let old = fixture.path().join("old");
	let new = fixture.path().join("new");
	let last = fixture.path().join("last");
	init_workspace(&old);
	fs::create_dir(old.join("projects")).unwrap();
	let api = repo(&old.join("projects"), "api");
	create(&old, "closed", &api);
	assert_ok(&workyard(&old, &["remove", "closed", "--yes"]));
	create(&old, "active", &api);

	fs::create_dir(&new).unwrap();
	for entry in [".workyard", "tasks"] {
		fs::rename(old.join(entry), new.join(entry)).unwrap();
	}
	assert_ok(&workyard(&new, &["recover"]));
	fs::rename(&new, &last).unwrap();
	assert_ok(&workyard(&last, &["recover"]));
	assert_eq!(
		registered(&api, "active"),
		last.join("tasks/active/api").canonicalize().unwrap()
	);
	let listed = workyard(&last, &["list"]);
	assert_ok(&listed);
	let text = String::from_utf8(listed.stdout).unwrap();
	assert!(text.contains("closed") && text.contains("active"), "{text}");
}

#[cfg(unix)]
#[test]
fn local_directory_aliases_cannot_remove_the_registered_original() {
	for relative in ["tasks/active/api", "tasks/active"] {
		let fixture = tempfile::tempdir().unwrap();
		let old = fixture.path().join("original");
		let new = fixture.path().join("copy");
		let api = repo(fixture.path(), "api");
		init_workspace(&old);
		create(&old, "active", &api);
		fs::write(old.join("tasks/active/api/file"), "original edits").unwrap();
		copy_directory(&old, &new);
		fs::remove_dir_all(new.join(relative)).unwrap();
		std::os::unix::fs::symlink(old.join(relative), new.join(relative)).unwrap();

		assert!(
			!workyard(&new, &["remove", "active", "--force"])
				.status
				.success()
		);
		assert!(
			!workyard(&new, &["recover", "--activate-here"])
				.status
				.success()
		);
		assert_eq!(
			fs::read(old.join("tasks/active/api/file")).unwrap(),
			b"original edits"
		);
		assert_eq!(
			registered(&api, "active"),
			old.join("tasks/active/api").canonicalize().unwrap()
		);
	}
}

#[cfg(unix)]
#[test]
fn unreachable_original_cannot_be_rebound_even_with_activation() {
	use std::os::unix::fs::PermissionsExt;

	let fixture = tempfile::tempdir().unwrap();
	let mount = fixture.path().join("mount");
	let old = mount.join("original");
	let new = fixture.path().join("copy");
	let api = repo(fixture.path(), "api");
	init_workspace(&old);
	create(&old, "active", &api);
	copy_directory(&old, &new);
	let registration = git(&api, &["worktree", "list", "--porcelain"]);
	let mode = fs::metadata(&mount).unwrap().permissions();
	fs::set_permissions(&mount, fs::Permissions::from_mode(0o000)).unwrap();
	let inaccessible = old.try_exists().is_err();
	let result = if inaccessible {
		Some(workyard(&new, &["recover", "--activate-here"]))
	} else {
		None
	};
	fs::set_permissions(&mount, mode).unwrap();
	if let Some(result) = result {
		assert!(!result.status.success());
		assert_eq!(
			registration,
			git(&api, &["worktree", "list", "--porcelain"])
		);
	}
}

#[test]
fn activation_rechecks_the_registered_copy_after_a_decision() {
	use workyard_core::Control;
	use workyard_git::Runner;
	use workyard_state::Reader;

	let fixture = tempfile::tempdir().unwrap();
	let old = fixture.path().join("old");
	let here = fixture.path().join("here");
	let third = fixture.path().join("third");
	let api = repo(fixture.path(), "api");
	init_workspace(&old);
	create(&old, "active", &api);
	copy_directory(&old, &here);
	copy_directory(&old, &third);

	let reader = Reader::open(&here.join(".workyard"), None).unwrap();
	let snapshot = reader.tasks(Some("active")).unwrap().pop().unwrap();
	let task = snapshot.task.unwrap();
	let participant = &task.participants[0];
	let runner = Runner::default();
	let control = Control::new(|| false);
	let preview = smol::block_on(runner.inspect_placement(participant, None, &control)).unwrap();
	assert!(preview.competing);

	let chosen = third.join("tasks/active/api");
	git(&api, &["worktree", "repair", text(&chosen)]);
	let result = smol::block_on(runner.apply_placement(
		participant,
		&preview,
		true,
		&Default::default(),
		&control,
	));
	assert!(result.is_err());
	assert_eq!(registered(&api, "active"), chosen.canonicalize().unwrap());
	for workspace in [&old, &here, &third] {
		assert!(workspace.join("tasks/active/api/.git").is_file());
	}
}

#[path = "portability/ownership.rs"]
mod ownership;

#[path = "portability/moves.rs"]
mod moves;
