use std::{
	io::Write,
	path::Path,
	process::{Command, Stdio},
};

pub fn invoke(root: &Path, action: &str, input: serde_json::Value) -> serde_json::Value {
	let mut child = Command::new(env!("CARGO_BIN_EXE_workyard"))
		.current_dir(root)
		.arg("rpc")
		.stdin(Stdio::piped())
		.stdout(Stdio::piped())
		.env("GIT_CONFIG_NOSYSTEM", "1")
		.env(
			"GIT_CONFIG_GLOBAL",
			if cfg!(windows) { "NUL" } else { "/dev/null" },
		)
		.spawn()
		.unwrap();
	let request = serde_json::json!({
		"jsonrpc":"2.0", "id":"test-1", "method":"invoke",
		"params":{"action":[action], "input": input}
	});
	writeln!(child.stdin.take().unwrap(), "{request}").unwrap();
	let output = child.wait_with_output().unwrap();
	serde_json::from_slice(&output.stdout).unwrap()
}
