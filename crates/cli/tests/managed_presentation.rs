//! Exercise the persisted presentation contract through the public CLI.

use std::{path::Path, process::Command};

fn run(root: &Path, args: &[&str]) -> String {
	let output = Command::new(env!("CARGO_BIN_EXE_workyard"))
		.current_dir(root)
		.env_remove("WORKYARD_CLI_PREFIX")
		.args(args)
		.output()
		.unwrap();
	assert!(
		output.status.success(),
		"{}",
		String::from_utf8_lossy(&output.stderr)
	);
	String::from_utf8(output.stdout).unwrap()
}

#[test]
fn hidden_settings_survive_reinitialization_and_shape_cli_help() {
	let temp = tempfile::tempdir().unwrap();
	let root = temp.path();
	let init_help = run(root, &["init", "--help"]);
	assert!(!init_help.contains("--display-name"));
	assert!(!init_help.contains("--invocation-prefix"));

	run(
		root,
		&[
			"init",
			"--display-name",
			"Example",
			"--invocation-prefix",
			"example task",
		],
	);
	let saved = root.join(".workyard/presentation.json");
	let original = std::fs::read(&saved).unwrap();
	run(root, &["init"]);
	assert_eq!(std::fs::read(&saved).unwrap(), original);

	let init_help = run(root, &["init", "--help"]);
	assert!(
		init_help.contains("Initialize the Example workspace"),
		"{init_help}"
	);
	let help = run(root, &["list", "--help"]);
	assert!(help.contains("Usage: example task list"), "{help}");
	assert!(!help.contains("--workspace"), "{help}");
	let result = run(root, &["list", "--json"]);
	serde_json::from_str::<serde_json::Value>(&result).unwrap();
	assert!(!result.contains("example task"));

	let listing = run(root, &["list", "--workspace", "."]);
	assert!(!listing.contains("  Example\n"), "{listing}");
	assert!(
		listing.contains("example task create feat/example <project>"),
		"{listing}"
	);
	assert!(!listing.contains("--workspace"), "{listing}");

	// A separate workspace in the same parent directory keeps standalone usage.
	let other = tempfile::tempdir().unwrap();
	run(other.path(), &["init"]);
	assert!(run(other.path(), &["list", "--help"]).contains("Usage: workyard list"));
	assert!(!other.path().join(".workyard/presentation.json").exists());
}
