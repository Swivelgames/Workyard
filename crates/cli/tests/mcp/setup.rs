use super::client::Client;
use serde_json::json;
use std::fs;

#[test]
fn withdrawing_init_releases_the_server_while_the_workspace_is_locked() {
	let fixture = tempfile::tempdir().unwrap();
	let root = fixture.path();
	let mut client = Client::launch(root);
	let initialized = client.call("init", json!({}));
	assert_ne!(initialized["isError"], true);
	let tasks = root.join("tasks");
	fs::remove_dir(&tasks).unwrap();
	let _gate =
		workyard_state::Lock::acquire(&root.join(".workyard/workspace.lock"), false).unwrap();

	let request = client.start_request("tools/call", json!({"name":"init", "arguments":{}}));
	let ping = client.request("ping", json!({}));
	assert_eq!(ping["result"], json!({}));
	client.cancel(request);
	client.close();

	assert!(!tasks.exists());
}

#[test]
fn init_reuses_the_selected_layout_and_rejects_conflicting_tasks_directories() {
	let fixture = tempfile::tempdir().unwrap();
	let root = fixture.path().canonicalize().unwrap();
	let workspace = root.join("workspace");
	let mut client = Client::launch(&root);

	let args = json!({"directory":"workspace", "tasks":"dev/worktrees"});
	let initialized = client.call("init", args.clone());
	assert_ne!(initialized["isError"], true);
	assert_eq!(
		initialized["structuredContent"]["workspace"],
		json!(workspace)
	);
	assert_eq!(
		initialized["structuredContent"]["tasks_root"],
		json!(workspace.join("dev/worktrees"))
	);
	assert!(workspace.join("dev/worktrees").is_dir());
	assert!(!root.join(".workyard").exists());

	let manifest = workspace.join(".workyard/workspace.json");
	let before = fs::read(&manifest).unwrap();
	assert_eq!(client.call("init", args), initialized);
	assert_eq!(
		client.call("init", json!({"directory":"workspace"})),
		initialized
	);

	let conflict = client.call(
		"init",
		json!({"directory":"workspace", "tasks":"elsewhere"}),
	);
	assert_eq!(conflict["isError"], true);
	assert_eq!(
		conflict["structuredContent"]["code"],
		"workspace_binding_conflict"
	);
	assert_eq!(fs::read(manifest).unwrap(), before);
	assert!(!workspace.join("elsewhere").exists());
	client.close();
}

#[test]
fn root_edits_validate_the_whole_request_and_allow_ordinary_retry() {
	let fixture = tempfile::tempdir().unwrap();
	let root = fixture.path().canonicalize().unwrap();
	let workspace = root.join("workspace");
	let mut client = Client::launch(&root);
	let args = json!({"workspace":"workspace", "paths":["future","future"]});

	for tool in ["projects.track", "projects.untrack"] {
		let missing = client.call(tool, args.clone());
		assert_eq!(missing["isError"], true);
		assert_eq!(missing["structuredContent"]["code"], "workspace_not_found");
	}
	assert!(!workspace.exists());
	let initialized = client.call("init", json!({"directory":"workspace"}));
	assert_ne!(initialized["isError"], true);

	fs::write(root.join("regular-file"), "keep").unwrap();
	let manifest = workspace.join(".workyard/workspace.json");
	let before = fs::read(&manifest).unwrap();
	for paths in [json!([]), json!(["future", "regular-file"])] {
		let failed = client.call(
			"projects.track",
			json!({"workspace":"workspace", "paths":paths}),
		);
		assert_eq!(failed["isError"], true);
		assert_eq!(failed["structuredContent"]["code"], "invalid_input");
		assert_eq!(fs::read(&manifest).unwrap(), before);
	}

	// Relative roots use the server's working directory even with an explicit workspace.
	let expected = root.join("future");
	for tool in ["projects.track", "projects.untrack"] {
		let edited = client.call(tool, args.clone());
		assert_ne!(edited["isError"], true);
		assert_eq!(edited["structuredContent"]["changed"], json!([expected]));

		let repeated = client.call(tool, args.clone());
		assert_ne!(repeated["isError"], true);
		assert_eq!(repeated["structuredContent"]["changed"], json!([]));
	}
	assert!(!expected.exists());
	client.close();
}
