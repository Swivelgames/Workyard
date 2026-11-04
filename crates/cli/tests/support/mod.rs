use std::{
	fs,
	path::{Path, PathBuf},
	process::{Command, Output},
};

pub fn git(path: &Path, args: &[&str]) -> String {
	let output = Command::new("git")
		.current_dir(path)
		.args(args)
		.env("GIT_CONFIG_NOSYSTEM", "1")
		.env(
			"GIT_CONFIG_GLOBAL",
			if cfg!(windows) { "NUL" } else { "/dev/null" },
		)
		.output()
		.unwrap();
	assert!(
		output.status.success(),
		"git {args:?}: {}",
		String::from_utf8_lossy(&output.stderr)
	);
	String::from_utf8(output.stdout).unwrap().trim().into()
}

pub fn repo(root: &Path, name: &str) -> PathBuf {
	let path = root.join(name);
	fs::create_dir(&path).unwrap();
	git(&path, &["init", "-b", "main"]);
	git(&path, &["config", "user.email", "fixture@example.invalid"]);
	git(&path, &["config", "user.name", "Fixture"]);
	fs::write(path.join("file"), "initial\n").unwrap();
	git(&path, &["add", "file"]);
	git(&path, &["commit", "-m", "initial"]);
	path
}

pub fn workyard(root: &Path, args: &[&str]) -> Output {
	Command::new(env!("CARGO_BIN_EXE_workyard"))
		.current_dir(root)
		.args(args)
		.env("GIT_CONFIG_NOSYSTEM", "1")
		.env(
			"GIT_CONFIG_GLOBAL",
			if cfg!(windows) { "NUL" } else { "/dev/null" },
		)
		.output()
		.unwrap()
}

pub fn text(path: &Path) -> &str {
	path.to_str().unwrap()
}

pub fn assert_ok(output: &Output) {
	assert!(
		output.status.success(),
		"stdout={} stderr={}",
		String::from_utf8_lossy(&output.stdout),
		String::from_utf8_lossy(&output.stderr)
	);
}

pub fn record(state: &Path) -> serde_json::Value {
	let path = fs::read_dir(state.join("tasks"))
		.unwrap()
		.next()
		.unwrap()
		.unwrap()
		.path();
	let bytes = fs::read(path).unwrap();
	assert!(bytes.ends_with(b"\n"));
	assert!(bytes.windows(2).any(|part| part == b"\n\t"));
	serde_json::from_slice(&bytes).unwrap()
}

pub fn remote_clone(root: &Path, name: &str) -> (PathBuf, PathBuf) {
	let upstream = repo(root, &format!("{name}-upstream"));
	let remote = root.join(format!("{name}.git"));
	git(root, &["clone", "--bare", text(&upstream), text(&remote)]);
	let clone = root.join(name);
	git(root, &["clone", text(&remote), text(&clone)]);
	(clone, remote)
}
