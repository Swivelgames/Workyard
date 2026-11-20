use super::{
	client::Client,
	coordination::{hold_fetch_lock, wait_for_effect, wait_until},
	support,
};
use serde_json::json;
use std::{fs, path::Path};

enum Interrupt {
	CancelCall,
	Disconnect,
}

#[test]
fn reusing_a_branch_requires_an_explicit_checkout_choice() {
	let fixture = tempfile::tempdir().unwrap();
	let workspace = fixture.path().join("workspace");
	support::init_workspace(&workspace);
	let (api, _) = support::remote_clone(fixture.path(), "api");
	let task = "feat/existing";
	support::git(&api, &["branch", "--track", task, "origin/main"]);

	let mut client = Client::launch(fixture.path());
	let mut args = json!({
		"workspace":workspace, "task":task, "repositories":[{"path":api}], "no_fetch":true
	});
	let guarded = client.call("create", args.clone());
	assert_eq!(guarded["isError"], true);
	assert_eq!(
		guarded["structuredContent"]["problem"]["code"],
		"branch_exists"
	);
	assert!(!workspace.join("tasks").join(task).exists());
	let listed = client.call("list", json!({"workspace":workspace}));
	assert_eq!(listed["structuredContent"]["tasks"], json!([]));

	args["checkout"] = true.into();
	args["no_fetch"] = false.into();

	let token = "checkout-existing";
	let id = client.start_observed("create", args, token);
	client.progress(token, 1, "Fetching api");
	client.progress(token, 2, "Creating worktree for api");
	client.progress(token, 3, "Created worktree for api");
	client.progress(token, 4, "Finished worktree setup for api");

	let created = client.reply(id)["result"].take();
	assert_ne!(created["isError"], true);
	assert_eq!(
		created["structuredContent"]["operation"]["outcome"],
		"succeeded"
	);
	assert_eq!(support::git(&api, &["branch", "--show-current"]), "main");
	client.close();
}

#[test]
fn disconnect_preserves_the_first_worktree_and_settles_before_host_exit() {
	interrupt_two_repository_create(Interrupt::Disconnect);
}

#[test]
fn cancelled_checkout_resumes_at_the_existing_branch_tip() {
	interrupt_two_repository_create(Interrupt::CancelCall);
}

#[test]
fn checkout_reports_live_progress_before_the_second_fetch_finishes() {
	let fixture = tempfile::tempdir().unwrap();
	let workspace = fixture.path().join("workspace");
	support::init_workspace(&workspace);
	let api = support::repo(fixture.path(), "api");
	let (web, _) = support::remote_clone(fixture.path(), "web");
	let lock = hold_fetch_lock(&web);

	let task = "feat/payments";
	let token = "checkout-payments";
	prepare_existing_branches(&api, &web, task);
	let args = json!({
		"task":task, "checkout":true, "from":"main", "repositories":[{"path":api},{"path":web}]
	});
	let mut client = Client::launch(&workspace);
	let id = client.start_observed("create", args, token);
	expect_first_worktree_and_fetch(&mut client, token);

	let root = workspace.join("tasks").join(task);
	assert!(root.join("api/.git").is_file());
	assert!(!root.join("web").exists());

	drop(lock);
	client.progress(token, 5, "Creating worktree for web");
	client.progress(token, 6, "Created worktree for web");
	client.progress(token, 7, "Finished worktree setup for web");

	let result = client.reply(id)["result"].take();
	let outcome = &result["structuredContent"];
	assert_eq!(outcome["lifecycle"], "active");
	assert_eq!(outcome["operation"]["outcome"], "succeeded");

	let participants = outcome["participants"].as_array().unwrap();
	assert_eq!(participants.len(), 2);
	assert_eq!(participants[0]["name"], "api");
	assert_eq!(participants[1]["name"], "web");

	let text = result["content"][0]["text"].as_str().unwrap();
	assert_eq!(
		serde_json::from_str::<serde_json::Value>(text).unwrap(),
		*outcome
	);
	client.close();
}

fn expect_first_worktree_and_fetch(client: &mut Client, token: &str) {
	client.progress(token, 1, "Creating worktree for api");
	client.progress(token, 2, "Created worktree for api");
	client.progress(token, 3, "Finished worktree setup for api");
	client.progress(token, 4, "Fetching web");
}

fn interrupt_two_repository_create(interrupt: Interrupt) {
	let fixture = tempfile::tempdir().unwrap();
	let workspace = fixture.path().join("workspace");
	support::init_workspace(&workspace);
	let projects = workspace.join("projects");
	fs::create_dir(&projects).unwrap();
	let api = support::repo(&projects, "api");
	let (web, _) = support::remote_clone(fixture.path(), "web");
	support::assert_ok(&support::workyard(
		&workspace,
		&["projects", "track", support::text(&web)],
	));

	// The second participant cannot fetch until this fixture releases the lock.
	let lock = hold_fetch_lock(&web);
	let task = "feat/payments";
	let mut client = Client::launch(&workspace);
	let checkout = matches!(interrupt, Interrupt::CancelCall);
	let mut arguments = json!({"task":task, "projects":["api","web"], "from":"main"});
	let expected_web_tip = if checkout {
		prepare_existing_branches(&api, &web, task);
		arguments["checkout"] = true.into();
		support::git(&web, &["rev-parse", task])
	} else {
		support::git(&web, &["rev-parse", "main"])
	};
	let token = "interrupt-create";
	let id = client.start_observed("create", arguments, token);
	expect_first_worktree_and_fetch(&mut client, token);
	wait_for_effect(&workspace, task, 1, "fetching");

	let task_root = workspace.join("tasks").join(task);
	let draft = task_root.join("api/draft");
	fs::write(&draft, "keep my work").unwrap();
	if let Interrupt::CancelCall = interrupt {
		client.cancel(id);
		wait_until("cancelled task to settle", || {
			let result = client.call("status", json!({"task":task}));
			let report = &result["structuredContent"]["tasks"][0];
			report["busy"] == false && report["recorded"]["operation"]["outcome"] == "cancelled"
		});
	}
	client.close();

	let mut next = Client::launch(&workspace);
	let status = next.call("status", json!({"task":task}));
	let report = &status["structuredContent"]["tasks"][0];
	assert_eq!(report["name"], task);
	assert_eq!(report["busy"], false);
	assert_eq!(report["recorded"]["operation"]["outcome"], "cancelled");
	assert!(report["pending_participant"].is_null());
	assert_eq!(report["current"][0]["name"], "api");
	assert_eq!(report["current"][0]["state"], "modified");
	assert_eq!(report["current"][1]["name"], "web");
	assert_eq!(report["current"][1]["state"], "missing");
	assert!(!task_root.join("web").exists());
	assert_eq!(
		!support::git(&web, &["branch", "--list", task]).is_empty(),
		checkout
	);

	// A second disconnect during recovery also leaves the task resumable.
	next.start_request(
		"tools/call",
		json!({"name":"recover","arguments":{"task":task}}),
	);
	wait_for_effect(&workspace, task, 1, "fetching");
	next.close();
	drop(lock);

	let mut resumed = Client::launch(&workspace);
	let recovered = resumed.call("recover", json!({"task":task}));
	assert_ne!(recovered["isError"], true, "{recovered}");
	let outcome = &recovered["structuredContent"];
	assert_eq!(outcome["operation"]["outcome"], "succeeded");
	assert_eq!(outcome["operation"]["continues"], "create");
	assert_eq!(outcome["nothing_to_recover"], false);
	assert!(task_root.join("web/.git").is_file());
	assert_eq!(
		support::git(&task_root.join("web"), &["branch", "--show-current"]),
		task
	);
	assert_eq!(
		support::git(&task_root.join("web"), &["rev-parse", "HEAD"]),
		expected_web_tip
	);
	assert_eq!(fs::read_to_string(draft).unwrap(), "keep my work");

	let repeated = resumed.call("recover", json!({"task":task}));
	assert_eq!(repeated["structuredContent"]["nothing_to_recover"], true);
	resumed.close();
}

fn prepare_existing_branches(api: &Path, web: &Path, task: &str) {
	support::git(api, &["branch", task]);
	// Preserve a target tip distinct from the local default used for new branches.
	support::git(web, &["branch", task]);
	support::git(
		web,
		&[
			"-c",
			"user.name=Fixture",
			"-c",
			"user.email=fixture@example.invalid",
			"commit",
			"--allow-empty",
			"-m",
			"Advance main",
		],
	);
}
