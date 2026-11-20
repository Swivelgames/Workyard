use super::{client::Client, coordination::hold_fetch_lock, support};
use serde_json::json;

#[test]
fn workspace_observations_remain_available_when_creates_fill_mutation_capacity() {
	let fixture = tempfile::tempdir().unwrap();
	let work = fixture.path().join("work");
	let idle = fixture.path().join("idle");
	support::init_workspace(&work);
	support::init_workspace(&idle);
	let (api, _) = support::remote_clone(fixture.path(), "api");
	let _gate = hold_fetch_lock(&api);

	let mut client = Client::launch(&work);
	let create = |index| {
		json!({
			"name": "create",
			"arguments": { "task": format!("feat/task-{index}"), "repositories": [{"path": api}] }
		})
	};
	for index in 0..14 {
		client.start_request("tools/call", create(index));
	}
	let overflow = client.request("tools/call", create(14));
	assert_eq!(overflow["error"]["code"], -32000, "{overflow}");

	for tool in ["status", "list", "info", "projects.list"] {
		let observed = client.call(tool, json!({"workspace": idle}));
		assert!(
			observed["structuredContent"].is_object(),
			"{tool}: {observed}"
		);
		assert_ne!(observed["isError"], true, "{tool}: {observed}");
	}
	client.close();
}
