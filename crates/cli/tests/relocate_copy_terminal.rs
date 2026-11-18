//! Exercise the actual terminal menu and the result returned by each selection.
#[cfg(unix)]
#[test]
fn recovery_menu_installs_the_selected_files() {
	use std::{io::ErrorKind, path::Path, process::Command};

	let script =
		Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/terminal_recovery_choice.py");
	let output = Command::new("python3")
		.env("PYTHONDONTWRITEBYTECODE", "1")
		.arg(script)
		.arg(env!("CARGO_BIN_EXE_workyard"))
		.output();
	let output = match output {
		Err(error) if error.kind() == ErrorKind::NotFound => {
			eprintln!("Skipping terminal recovery smoke test because python3 is unavailable.");
			return;
		}
		other => other.expect("run terminal recovery smoke test"),
	};
	assert!(
		output.status.success(),
		"{}",
		String::from_utf8_lossy(&output.stderr)
	);
}
