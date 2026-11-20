//! Retired options fail at the interface boundary, before any action can run.
#[path = "support/rpc.rs"]
mod rpc;
#[allow(dead_code)]
mod support;

use serde_json::{Value, json};

#[test]
fn cli_rejects_retired_pull_options_and_omits_them_from_help() {
	let fixture = tempfile::tempdir().unwrap();
	for (action, flag) in [
		("create", "--pull"),
		("checkout", "--pull"),
		("amend", "--pull"),
		("recover", "--accept-pull"),
	] {
		let help = support::workyard(fixture.path(), &[action, "--help"]);
		support::assert_ok(&help);
		assert!(!String::from_utf8(help.stdout).unwrap().contains(flag));

		let mut args = vec![action, "topic", flag];
		if action == "recover" {
			args.push("api");
		}
		args.push("--json");
		let output = support::workyard(fixture.path(), &args);
		assert!(!output.status.success());
		let document: Value = serde_json::from_slice(&output.stdout).unwrap();
		assert_eq!(document["error"]["code"], "invalid_input", "{document}");
		assert!(document["result"].is_null());
		assert!(
			document["error"]["message"]
				.as_str()
				.unwrap()
				.contains(flag)
		);
	}
}

#[test]
fn rpc_rejects_retired_pull_fields_before_action_execution() {
	let fixture = tempfile::tempdir().unwrap();
	for (action, input) in [
		("create", json!({"task":"topic","pull":true})),
		("amend", json!({"task":"topic","pull":true})),
		("recover", json!({"task":"topic","accept_pull":["api"]})),
	] {
		let reply = rpc::invoke(fixture.path(), action, input);
		assert_eq!(reply["error"]["code"], -32602, "{reply}");
		assert!(reply.get("result").is_none(), "{reply}");
	}
}
