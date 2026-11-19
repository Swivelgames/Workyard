use crate::support::{assert_ok, workyard};
use serde_json::{Value, json};
use std::{
	collections::BTreeMap,
	fs,
	io::Write,
	path::{Path, PathBuf},
	process::{Command, Output, Stdio},
};

pub(super) fn cli_result(output: &Output) -> Value {
	serde_json::from_slice::<Value>(&output.stdout).unwrap()["result"].take()
}

pub(super) fn inventory(workspace: &Path) -> Value {
	let output = workyard(workspace, &["projects", "list", "--json"]);
	assert_ok(&output);
	cli_result(&output)
}

pub(super) fn references(inventory: &Value) -> Vec<&str> {
	inventory["projects"]
		.as_array()
		.unwrap()
		.iter()
		.map(|project| project["reference"].as_str().unwrap())
		.collect()
}

pub(super) fn invoke(cwd: &Path, action: &[&str], input: Value) -> Value {
	let mut host = Command::new(env!("CARGO_BIN_EXE_workyard"))
		.current_dir(cwd)
		.arg("rpc")
		.env("GIT_CONFIG_NOSYSTEM", "1")
		.env(
			"GIT_CONFIG_GLOBAL",
			if cfg!(windows) { "NUL" } else { "/dev/null" },
		)
		.stdin(Stdio::piped())
		.stdout(Stdio::piped())
		.spawn()
		.unwrap();

	let request = json!({
		"jsonrpc": "2.0", "id": "projects-test", "method": "invoke",
		"params": { "action": action, "input": input }
	});
	writeln!(host.stdin.take().unwrap(), "{request}").unwrap();

	let output = host.wait_with_output().unwrap();
	assert_ok(&output);
	serde_json::from_slice(&output.stdout).unwrap()
}

pub(super) fn complete(workspace: &Path, arguments: &[&str]) -> Vec<String> {
	let output = Command::new(env!("CARGO_BIN_EXE_workyard"))
		.current_dir(workspace)
		.env("TRINE_COMPLETE", "zsh")
		.env("_CLAP_COMPLETE_INDEX", arguments.len().to_string())
		.env_remove("TRINE_COMPLETE_LINE")
		.env_remove("TRINE_COMPLETE_METADATA")
		.args(["--", "workyard"])
		.args(arguments)
		.output()
		.unwrap();

	assert_ok(&output);
	String::from_utf8(output.stdout)
		.unwrap()
		.lines()
		.map(str::to_owned)
		.collect()
}

pub(super) fn snapshot(directory: &Path) -> BTreeMap<PathBuf, Vec<u8>> {
	let mut entries = BTreeMap::new();
	for entry in fs::read_dir(directory).unwrap() {
		let path = entry.unwrap().path();
		if path.is_dir() {
			entries.insert(path.clone(), Vec::new());
			entries.extend(snapshot(&path));
		} else {
			entries.insert(path.clone(), fs::read(path).unwrap());
		}
	}
	entries
}

/// Let discovery observe the original clone. The next inspection, either a grouped
/// override or core preflight, sees a different clone at the same path.
#[cfg(unix)]
pub(super) fn swap_clone_on_inspection(
	workspace: &Path,
	source: &Path,
	replacement: &Path,
) -> Command {
	use std::{env, os::unix::fs::PermissionsExt};

	let tools = workspace.join("bin");
	fs::create_dir(&tools).unwrap();
	let shim = tools.join("git");
	fs::write(
		&shim,
		r#"#!/bin/sh
set -eu
if [ "$PWD" = "$SELECTED" ] && [ -d "$REPLACEMENT" ]; then
	case "$*" in
		*--git-common-dir)
			if [ -e "$REPLACEMENT-seen" ]; then
				mv "$SELECTED" "$REPLACEMENT-original"
				mv "$REPLACEMENT" "$SELECTED"
				cd "$SELECTED"
			else
				touch "$REPLACEMENT-seen"
			fi
			;;
	esac
fi
PATH="$REAL_PATH" exec git "$@"
"#,
	)
	.unwrap();
	fs::set_permissions(&shim, fs::Permissions::from_mode(0o700)).unwrap();

	let original_path = env::var_os("PATH").unwrap();
	let path =
		env::join_paths(std::iter::once(tools).chain(env::split_paths(&original_path))).unwrap();
	let mut command = Command::new(env!("CARGO_BIN_EXE_workyard"));
	command
		.current_dir(workspace)
		.env("PATH", path)
		.env("REAL_PATH", original_path)
		.env("SELECTED", source.canonicalize().unwrap())
		.env("REPLACEMENT", replacement.canonicalize().unwrap())
		.env("GIT_CONFIG_GLOBAL", "/dev/null")
		.env("GIT_CONFIG_NOSYSTEM", "1");
	command
}
