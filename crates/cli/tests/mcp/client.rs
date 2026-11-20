use super::support::assert_ok;
use serde_json::{Value, json};
use smol::{
	io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
	process::{Child, ChildStdout, Command, Stdio},
};
use std::{future::Future, io, path::Path, time::Duration};

pub(super) struct Client {
	child: Child,
	replies: BufReader<ChildStdout>,
	next_id: u32,
}

impl Client {
	pub(super) fn launch(cwd: &Path) -> Self {
		let mut child = Command::new(env!("CARGO_BIN_EXE_workyard"))
			.kill_on_drop(true)
			.env("GIT_CONFIG_NOSYSTEM", "1")
			.env(
				"GIT_CONFIG_GLOBAL",
				if cfg!(windows) { "NUL" } else { "/dev/null" },
			)
			.arg("mcp")
			.current_dir(cwd)
			.stdin(Stdio::piped())
			.stdout(Stdio::piped())
			.stderr(Stdio::piped())
			.spawn()
			.unwrap();

		let replies = BufReader::new(child.stdout.take().unwrap());
		let mut client = Self {
			child,
			replies,
			next_id: 1,
		};

		let init = client.request(
			"initialize",
			json!({
				"protocolVersion": "2025-11-25", "capabilities": {},
				"clientInfo": {"name":"fixture","version":"1"}
			}),
		);
		assert_eq!(init["result"]["serverInfo"]["name"], "workyard");
		client.send(json!({"jsonrpc":"2.0","method":"notifications/initialized"}));
		client
	}

	fn send(&mut self, message: Value) {
		let frame = format!("{message}\n");
		let input = self.child.stdin.as_mut().unwrap();
		within_deadline(input.write_all(frame.as_bytes()));
	}

	pub(super) fn start_request(&mut self, method: &str, params: Value) -> u32 {
		let id = self.next_id;
		self.next_id += 1;
		self.send(json!({"jsonrpc":"2.0","id":id,"method":method,"params":params}));
		id
	}

	pub(super) fn start_observed(&mut self, tool: &str, args: Value, token: &str) -> u32 {
		self.start_request(
			"tools/call",
			json!({"name":tool, "arguments":args, "_meta":{"progressToken":token}}),
		)
	}

	pub(super) fn cancel(&mut self, id: u32) {
		self.send(
			json!({"jsonrpc":"2.0","method":"notifications/cancelled","params":{"requestId":id}}),
		);
	}

	pub(super) fn request(&mut self, method: &str, params: Value) -> Value {
		let id = self.start_request(method, params);
		self.reply(id)
	}

	fn frame(&mut self) -> Value {
		let mut line = String::new();
		within_deadline(self.replies.read_line(&mut line));
		serde_json::from_str(&line).unwrap()
	}

	pub(super) fn progress(&mut self, token: &str, number: u32, message: &str) {
		assert_eq!(
			self.frame(),
			json!({"jsonrpc":"2.0","method":"notifications/progress","params":{
				"progressToken":token, "progress":number, "message":message
			}})
		);
	}

	pub(super) fn reply(&mut self, id: u32) -> Value {
		let reply = self.frame();
		assert_eq!(reply["id"], id);
		reply
	}

	pub(super) fn call(&mut self, tool: &str, args: Value) -> Value {
		self.request("tools/call", json!({"name":tool,"arguments":args}))["result"].take()
	}

	pub(super) fn close(mut self) {
		drop(self.child.stdin.take());
		let mut trailing = String::new();
		let remaining = within_deadline(self.replies.read_line(&mut trailing));
		assert_eq!(remaining, 0, "{trailing}");

		let output = within_deadline(self.child.output());
		assert_ok(&output);
		assert!(output.stderr.is_empty());
	}
}

// A wedged server should fail its test even while both ends of the pipe are open.
fn within_deadline<T>(operation: impl Future<Output = io::Result<T>>) -> T {
	let timeout = async {
		smol::Timer::after(Duration::from_secs(15)).await;
		Err(io::Error::new(
			io::ErrorKind::TimedOut,
			"Workyard MCP test client timed out",
		))
	};
	smol::block_on(smol::future::or(operation, timeout)).unwrap()
}
