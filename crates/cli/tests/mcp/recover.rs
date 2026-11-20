use super::{
	client::Client,
	coordination::{StopAtPhase, task_documents},
	support,
};
use serde_json::json;
use std::{fs, path::Path};
use workyard_core::{Control, OperationOutcome, RemovalOptions};

#[test]
fn requires_a_task_and_rejects_administrative_options() {
	let fixture = tempfile::tempdir().unwrap();
	let workspace = fixture.path();
	support::init_workspace(workspace);
	let mut client = Client::launch(workspace);
	for arguments in [
		json!({}),
		json!({"task":null}),
		json!({"activate_here":true}),
		json!({"task":"feat/one","undo":true}),
		json!({"task":"feat/one","force":true}),
		json!({"task":"feat/one","yes":true}),
	] {
		let reply = client.call("recover", arguments);
		assert_eq!(reply["isError"], true);
		assert!(reply["structuredContent"].is_null(), "{reply}");
	}
	let unknown = client.call(
		"recover",
		json!({"workspace":workspace, "task":"feat/unknown"}),
	);
	assert_eq!(unknown["isError"], true);
	assert_eq!(
		unknown["structuredContent"]["problem"]["code"],
		"task_unknown"
	);
	client.close();
}

#[test]
fn named_recovery_reports_link_repair_and_location_updates_only_when_needed() {
	let fixture = tempfile::tempdir().unwrap();
	let workspace = fixture.path().join("workspace");
	support::init_workspace(&workspace);
	let api = support::repo(fixture.path(), "api");
	let task = "feat/locations";
	let task_root = workspace.join("tasks").join(task);
	let mut client = Client::launch(&workspace);
	let created = client.call(
		"create",
		json!({"task":task, "repositories":[{"path":api}], "from":"main", "no_fetch":true}),
	);
	assert_ne!(created["isError"], true, "{created}");

	// A filesystem rename leaves Git pointing at the old worktree.
	let renamed = task_root.join("renamed");
	fs::rename(task_root.join("api"), &renamed).unwrap();
	fs::write(renamed.join("draft"), "keep my edits").unwrap();
	let token = "repair-link";
	let id = client.start_observed("recover", json!({"task":task}), token);
	client.progress(token, 1, "Repairing Git worktree link for api");
	let repaired = client.reply(id)["result"].take();
	assert_eq!(repaired["structuredContent"]["nothing_to_recover"], false);
	assert_eq!(support::git(&renamed, &["branch", "--show-current"]), task);

	// A move through Git already has a valid link. Only the recorded location changes.
	let moved = task_root.join("moved");
	support::git(
		&api,
		&[
			"worktree",
			"move",
			support::text(&renamed),
			support::text(&moved),
		],
	);
	let id = client.start_observed("recover", json!({"task":task}), token);
	client.progress(token, 1, "Updating saved worktree location for api");
	let updated = client.reply(id)["result"].take();
	assert_eq!(updated["structuredContent"]["nothing_to_recover"], false);
	assert_eq!(
		fs::read_to_string(moved.join("draft")).unwrap(),
		"keep my edits"
	);

	let state = workspace.join(".workyard");
	let before = task_documents(&state);
	let id = client.start_observed("recover", json!({"task":task}), token);
	let settled = client.reply(id)["result"].take();
	assert_eq!(settled["structuredContent"]["nothing_to_recover"], true);
	assert_eq!(before, task_documents(&state));
	client.close();
}

#[test]
fn uses_the_membership_revision_after_amendment_and_refuses_whole_task_removal() {
	let fixture = tempfile::tempdir().unwrap();
	let workspace = fixture.path().join("workspace");
	support::init_workspace(&workspace);
	let task = "feat/other-operation";
	let api = support::repo(fixture.path(), "api");
	let web = support::repo(fixture.path(), "web");
	let state = workspace.join(".workyard");
	let mut client = Client::launch(&workspace);

	support::assert_ok(&support::workyard(
		&workspace,
		&[
			"create",
			task,
			"--repo",
			support::text(&api),
			"--from",
			"main",
		],
	));
	let published = state
		.join("tasks")
		.join(workyard_state::task_filename(task));
	let creation_record = fs::read(&published).unwrap();
	support::assert_ok(&support::workyard(
		&workspace,
		&[
			"amend",
			task,
			"--repo",
			support::text(&web),
			"--from",
			"main",
		],
	));
	// Model a crash before the published task catches up with its membership revision.
	fs::write(&published, creation_record).unwrap();
	let reply = client.call("recover", json!({"task":task}));
	assert_ne!(reply["isError"], true, "{reply}");
	assert_eq!(
		reply["structuredContent"]["operation"]["continues"],
		"amend"
	);
	assert_eq!(
		support::record(&state)["participants"]
			.as_array()
			.unwrap()
			.len(),
		2
	);

	let store = workyard_state::Store::open_existing(&state, None).unwrap();
	let interrupted = smol::block_on(workyard_core::remove(
		task,
		RemovalOptions {
			yes: true,
			..Default::default()
		},
		&workyard_git::Runner::default(),
		&Control::default(),
		&StopAtPhase("removed"),
		&store,
	))
	.unwrap();
	assert_eq!(interrupted.operation.outcome, OperationOutcome::Cancelled);
	let trees = workspace.join("tasks").join(task);
	let remaining = [trees.join("api"), trees.join("web")]
		.into_iter()
		.filter(|path| path.exists())
		.collect::<Vec<_>>();
	assert_eq!(remaining.len(), 1);
	let draft = remaining[0].join("draft");
	fs::write(&draft, "keep this worktree").unwrap();
	assert_recovery_refused(&mut client, task, &state);
	assert_eq!(fs::read_to_string(draft).unwrap(), "keep this worktree");
	client.close();
}

fn assert_recovery_refused(client: &mut Client, task: &str, state: &Path) {
	let before = task_documents(state);
	let id = client.start_observed("recover", json!({"task":task}), "ineligible");
	let reply = client.reply(id)["result"].take();
	assert_eq!(reply["isError"], true, "{reply}");
	assert_eq!(
		reply["structuredContent"]["problem"]["code"],
		"invalid_input"
	);
	assert_eq!(task_documents(state), before);
}
