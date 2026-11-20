use super::{
	client::Client,
	coordination::{StopAtPhase, hold_fetch_lock, task_documents, wait_until},
	support,
};
use serde_json::{Value, json};
use std::{fs, path::Path};
use workyard_core::{AmendRequest, Control, CreationOptions, OperationOutcome, RemovalOptions};

#[test]
fn empty_amendment_explains_how_to_select_participants() {
	let fixture = tempfile::tempdir().unwrap();
	let mut client = Client::launch(fixture.path());
	let reply = client.call("amend", json!({"task":"feat/payments"}));
	let problem = &reply["structuredContent"]["problem"];

	assert_eq!(reply["isError"], true);
	assert_eq!(problem["code"], "invalid_input");
	let message = problem["message"].as_str().unwrap();
	assert!(
		message.contains("project") && message.contains("remov"),
		"{message}"
	);
	client.close();
}

#[test]
fn disconnected_amendment_resumes_after_a_rename_without_replacing_authored_files() {
	let fixture = tempfile::tempdir().unwrap();
	let workspace = fixture.path().join("workspace");
	support::init_workspace(&workspace);
	let api = support::repo(fixture.path(), "api");
	let (infra, _) = support::remote_clone(fixture.path(), "infra");
	let task = "feat/payments";
	let trees = workspace.join("tasks").join(task);
	let state = workspace.join(".workyard");

	let mut client = Client::launch(&workspace);
	let created = client.call(
		"create",
		json!({"task":task,"repositories":[{"path":api}],"from":"main"}),
	);
	assert_ne!(created["isError"], true, "{created}");
	fs::write(trees.join("api/draft"), "unfinished work").unwrap();
	let original_identity = support::record(&state)["participants"][0]["worktree_identity"].clone();
	assert!(original_identity.is_string());

	let lock = hold_fetch_lock(&infra);
	let args = json!({"task":task,"repositories":[{"path":api,"name":"backend"},{"path":infra}],"from":"main"});
	let token = "amend-payments";
	client.start_observed("amend", args, token);
	client.progress(token, 1, "Renamed participant to backend");
	client.progress(token, 2, "Fetching infra");
	assert!(trees.join("backend/draft").is_file());
	assert!(!trees.join("infra").exists());

	let revision = state
		.join("evidence")
		.join(workyard_state::task_filename(task))
		.with_extension("revision");
	wait_until("amendment waiting to fetch infra", || {
		let Ok(bytes) = fs::read(&revision) else {
			return false;
		};
		let document: Value = serde_json::from_slice(&bytes).unwrap();
		let current = &document["current"];
		current["pending"] == 1 && current["task"]["participants"][1]["phase"] == "fetching"
	});
	client.close();
	assert!(!trees.join("infra").exists());

	let mut client = Client::launch(&workspace);
	let status = client.call("status", json!({"task":task}));
	assert_ne!(status["isError"], true);
	let interrupted = &status["structuredContent"]["tasks"][0]["recorded"]["operation"];
	assert_eq!(interrupted["kind"], "amend");
	assert_eq!(interrupted["outcome"], "cancelled");

	let token = "recover-payments";
	let id = client.start_observed("recover", json!({"task":task}), token);
	client.progress(token, 1, "Checked participant backend (unchanged)");
	client.progress(token, 2, "Fetching infra");
	assert!(!trees.join("infra").exists());
	drop(lock);

	client.progress(token, 3, "Creating worktree for infra");
	client.progress(token, 4, "Added participant infra");
	let recovered = client.reply(id)["result"].take();
	assert_ne!(recovered["isError"], true, "{recovered}");
	let outcome = &recovered["structuredContent"];
	assert_eq!(outcome["operation"]["continues"], "amend");
	assert_eq!(outcome["operation"]["outcome"], "succeeded");
	assert!(
		outcome["changes"]
			.as_array()
			.unwrap()
			.contains(&json!({"participant":"infra","kind":"added"}))
	);

	assert_eq!(
		fs::read_to_string(trees.join("backend/draft")).unwrap(),
		"unfinished work"
	);
	assert!(!trees.join("api").exists());
	assert_eq!(
		support::record(&state)["participants"][0]["worktree_identity"],
		original_identity
	);
	assert_eq!(
		support::git(&trees.join("infra"), &["branch", "--show-current"]),
		task
	);
	let settled = task_documents(&state);
	let repeat = client.call("recover", json!({"task":task}));
	assert_eq!(
		repeat["structuredContent"]["nothing_to_recover"], true,
		"{repeat}"
	);
	assert_eq!(task_documents(&state), settled);
	client.close();
}

#[test]
fn an_unrelated_amendment_cannot_resume_removal_from_a_stale_task_record() {
	let fixture = tempfile::tempdir().unwrap();
	let workspace = fixture.path().join("workspace");
	support::init_workspace(&workspace);
	let task = "feat/membership";
	let state = workspace.join(".workyard");
	let trees = workspace.join("tasks").join(task);
	let mut client = Client::launch(&workspace);
	let repositories =
		["api", "web", "infra"].map(|name| json!({"path":support::repo(fixture.path(), name)}));
	let created = client.call(
		"create",
		json!({"task":task,"repositories":repositories,"from":"main"}),
	);
	assert_ne!(created["isError"], true, "{created}");
	let published = state
		.join("tasks")
		.join(workyard_state::task_filename(task));
	let original = fs::read(&published).unwrap();
	let worker = support::repo(fixture.path(), "worker");
	let before = task_documents(&state);
	for name in ["force", "yes"] {
		let mut args = json!({"task":task, "repositories":[{"path":worker}]});
		args[name] = json!(true);
		let refused = client.call("amend", args);
		assert_eq!(refused["isError"], true, "{refused}");
		assert!(refused["structuredContent"].is_null());
		assert_eq!(task_documents(&state), before);
	}

	interrupt_participant_removal(task, &state);
	assert_ne!(trees.join("infra").exists(), trees.join("web").exists());
	let remaining = if trees.join("infra").exists() {
		"infra"
	} else {
		"web"
	};
	fs::write(trees.join("api/draft"), "keep me").unwrap();
	fs::write(&published, original).unwrap();

	let pending = task_documents(&state);
	for (name, args) in [
		(
			"amend",
			json!({"task":task,"repositories":[{"path":worker}]}),
		),
		("recover", json!({"task":task})),
	] {
		let refused = client.call(name, args);
		assert_eq!(refused["isError"], true, "{refused}");
		assert_eq!(
			refused["structuredContent"]["problem"]["code"],
			"invalid_input"
		);
		assert_eq!(task_documents(&state), pending);
		assert!(trees.join(remaining).join(".git").is_file());
		assert!(!trees.join("worker").exists());
	}

	support::assert_ok(&support::workyard(&workspace, &["recover", task]));
	let added = client.call(
		"amend",
		json!({"task":task,"repositories":[{"path":worker}], "from":"main"}),
	);
	assert_ne!(added["isError"], true, "{added}");
	assert!(trees.join("worker/.git").is_file());
	assert_eq!(
		fs::read_to_string(trees.join("api/draft")).unwrap(),
		"keep me"
	);
	client.close();
}

fn interrupt_participant_removal(task: &str, state: &Path) {
	let store = workyard_state::Store::open_existing(state, None).unwrap();
	let removal = AmendRequest {
		task: task.into(),
		repositories: Vec::new(),
		remove: vec!["web".into(), "infra".into()],
		removal_options: RemovalOptions::default(),
		options: CreationOptions {
			checkout: false,
			no_fetch: true,
		},
		from: None,
		remote: None,
	};
	let interrupted = smol::block_on(workyard_core::amend(
		removal,
		&workyard_git::Runner::default(),
		&Control::default(),
		&StopAtPhase("removed"),
		&store,
	))
	.unwrap();
	assert_eq!(interrupted.operation.outcome, OperationOutcome::Cancelled);
}
