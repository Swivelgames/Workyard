#[path = "mcp/amend.rs"]
mod amend;
#[path = "mcp/amend_settlement.rs"]
mod amend_settlement;
#[path = "mcp/capacity.rs"]
mod capacity;
#[path = "mcp/client.rs"]
mod client;
#[path = "mcp/coordination.rs"]
mod coordination;
#[path = "mcp/create.rs"]
mod create;
#[path = "mcp/recover.rs"]
mod recover;
#[path = "mcp/removal/mod.rs"]
mod removal;
#[path = "mcp/setup.rs"]
mod setup;
#[allow(dead_code)]
mod support;

use client::Client;
use serde_json::json;
use std::fs;
use support::{assert_ok, init_workspace, repo, text, workyard};

#[test]
fn discovers_workspace_tools_with_native_workspace_paths() {
	let fixture = tempfile::tempdir().unwrap();
	let workspace = fixture.path().canonicalize().unwrap().join("workspace");
	init_workspace(&workspace);
	let mut client = Client::launch(fixture.path());

	let inventory = client.request("tools/list", json!({}));
	let tools = inventory["result"]["tools"].as_array().unwrap();
	let names: Vec<_> = tools
		.iter()
		.map(|tool| tool["name"].as_str().unwrap())
		.collect();
	assert_eq!(
		names,
		[
			"amend",
			"create",
			"info",
			"init",
			"list",
			"projects.list",
			"projects.track",
			"projects.untrack",
			"recover",
			"remove",
			"status"
		]
	);

	for tool in tools {
		let schema = &tool["inputSchema"];
		let fields = &schema["properties"];
		let name = tool["name"].as_str().unwrap();
		assert!(fields.get("pull").is_none(), "{name}");
		assert!(fields.get("accept_pull").is_none(), "{name}");
		if name == "init" {
			assert!(fields["directory"].is_object());
			assert!(fields["tasks"].is_object());
			assert!(fields.get("workspace").is_none());
		} else {
			assert!(fields["workspace"].is_object(), "{name}");
		}
		if matches!(name, "projects.track" | "projects.untrack") {
			assert_eq!(schema["required"], json!(["paths"]));
			assert_eq!(fields["paths"]["type"], "array");
			assert_eq!(fields["paths"]["minItems"], 1);
		}

		assert_eq!(
			fields.get("task").is_some(),
			matches!(name, "amend" | "create" | "recover" | "remove" | "status")
		);
		if matches!(name, "amend" | "remove") {
			assert_eq!(schema["required"], json!(["task"]));
			assert_eq!(schema["additionalProperties"], false);
			assert!(fields.get("approve_discard").is_some());
			assert_eq!(fields.get("remove").is_some(), name == "amend");
			for excluded in ["force", "yes", "undo"] {
				assert!(fields.get(excluded).is_none());
			}
		}
		if name == "recover" {
			assert_eq!(schema["required"], json!(["task"]));
			assert_eq!(fields.as_object().unwrap().len(), 2);
			assert_eq!(schema["additionalProperties"], false);
		}

		// Native path forms can be referenced from the schema's definitions.
		let schema_json = schema.to_string();
		for field in ["platform", "bytes", "wide"] {
			assert!(schema_json.contains(&format!("\"{field}\"")), "{field}");
		}
	}

	#[cfg(unix)]
	let path = json!({"platform":"unix","bytes":"d29ya3NwYWNl"});
	#[cfg(windows)]
	let path = json!({"platform":"windows","wide":"workspace".encode_utf16().collect::<Vec<_>>()});
	for tool in ["info", "list", "projects.list", "status"] {
		let encoded = client.call(tool, json!({"workspace":path}));
		let plain = client.call(tool, json!({"workspace":"workspace"}));
		assert_eq!(encoded, plain, "{tool}");
		let result = &encoded["structuredContent"];
		if tool == "projects.list" {
			assert_eq!(
				result["roots"][0]["path"],
				json!(workspace.join("projects"))
			);
		} else {
			assert_eq!(
				result["tasks_root"],
				json!(workspace.join("tasks")),
				"{tool}"
			);
		}
	}
	client.close();
}

#[test]
fn retired_pull_options_are_rejected_before_tool_execution() {
	let fixture = tempfile::tempdir().unwrap();
	let mut client = Client::launch(fixture.path());
	for (action, args) in [
		("create", json!({"task":"topic","pull":true})),
		("amend", json!({"task":"topic","pull":true})),
		("recover", json!({"task":"topic","accept_pull":["api"]})),
	] {
		let error = client.call(action, args);
		assert_eq!(error["isError"], true, "{action}: {error}");
		assert!(error["structuredContent"].is_null(), "{action}: {error}");
	}
	client.close();
}

#[test]
fn observation_tools_leave_absent_workspaces_uninitialized() {
	let fixture = tempfile::tempdir().unwrap();
	let mut client = Client::launch(fixture.path());
	for (tool, field, expected) in [
		("list", "tasks", json!([])),
		("status", "tasks", json!([])),
		("info", "active_tasks", json!(0)),
		("projects.list", "projects", json!([])),
	] {
		let empty = client.call(tool, json!({}));
		let observation = &empty["structuredContent"];
		assert_ne!(empty["isError"], true, "{tool}");
		assert_eq!(observation[field], expected, "{tool}");
		assert!(
			observation["guidance"]
				.as_str()
				.unwrap()
				.contains("workyard init")
		);
	}
	assert!(!fixture.path().join(".workyard").exists());
	client.close();
}

#[cfg(unix)]
#[test]
fn project_inventory_returns_lossless_repository_references_and_paths() {
	use std::{ffi::OsStr, os::unix::ffi::OsStrExt, path::Path};
	use trine::NativePath;

	let fixture = tempfile::tempdir().unwrap();
	let workspace = fixture.path().canonicalize().unwrap();
	init_workspace(&workspace);
	let projects = workspace.join("projects");
	fs::create_dir(&projects).unwrap();
	let reference = OsStr::from_bytes(b"api-\xff");
	let location = projects.join(reference);
	fs::rename(repo(&projects, "api"), &location).unwrap();

	let mut client = Client::launch(&workspace);
	let response = client.call("projects.list", json!({}));
	assert_ne!(response["isError"], true);
	let discovered = &response["structuredContent"]["projects"][0];
	let name: NativePath = serde_json::from_value(discovered["reference"].clone()).unwrap();
	let path: NativePath = serde_json::from_value(discovered["path"].clone()).unwrap();
	assert_eq!(name.as_path(), Path::new(reference));
	assert_eq!(path.as_path(), location);
	client.close();
}

#[test]
fn damaged_records_and_workspace_metadata_remain_inspectable() {
	let fixture = tempfile::tempdir().unwrap();
	let mut client = Client::launch(fixture.path());
	let workspace = fixture.path().join("workspace");
	let source = repo(fixture.path(), "api");
	init_workspace(&workspace);
	assert_ok(&workyard(
		&workspace,
		&[
			"create",
			"--no-fetch",
			"--from",
			"main",
			"--repo",
			text(&source),
			"feat/api",
		],
	));
	let state = workspace.join(".workyard");
	let record = state
		.join("tasks")
		.join(workyard_state::task_filename("feat/api"));
	fs::write(&record, "damaged").unwrap();

	let inventory = client.call("list", json!({"workspace":workspace}));
	assert_ne!(inventory["isError"], true);
	let tasks = inventory["structuredContent"]["tasks"].as_array().unwrap();
	assert_eq!(tasks.len(), 1);
	assert!(!tasks[0]["problems"].as_array().unwrap().is_empty());
	assert_eq!(fs::read_to_string(record).unwrap(), "damaged");

	fs::write(state.join("workspace.json"), "damaged").unwrap();
	for tool in ["list", "status", "info"] {
		let failed = client.call(tool, json!({"workspace":workspace}));
		assert_eq!(failed["isError"], true, "{tool}");
		assert_eq!(
			failed["structuredContent"]["problem"]["code"], "damaged_state",
			"{tool}"
		);
	}

	// Project discovery returns its typed problem directly.
	let projects = client.call("projects.list", json!({"workspace":workspace}));
	assert_eq!(projects["isError"], true);
	assert_eq!(projects["structuredContent"]["code"], "damaged_state");
	client.close();
}

#[test]
fn json_flag_cannot_put_cli_output_on_the_mcp_transport() {
	let fixture = tempfile::tempdir().unwrap();
	for args in [["mcp", "--json"], ["--json", "mcp"]] {
		let output = workyard(fixture.path(), &args);
		assert!(!output.status.success());
		assert!(output.stdout.is_empty());
		assert!(
			String::from_utf8(output.stderr)
				.unwrap()
				.contains("--json cannot be combined with mcp")
		);
	}
}
