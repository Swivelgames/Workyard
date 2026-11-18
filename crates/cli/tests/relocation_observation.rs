#[allow(dead_code)]
mod support;

use serde_json::{Value, json};
use std::{fs, path::Path};
use support::{assert_ok, repo, text, workyard};
use workyard_core::RelocationState;
use workyard_state::Relocation;

#[test]
fn forward_cleanup_reports_destination_from_either_endpoint() {
	check_observation(false, false);
}

#[test]
fn undo_cleanup_reports_source_from_either_endpoint() {
	check_observation(true, false);
}

#[test]
fn forward_cleanup_reports_new_root_with_stationary_state() {
	check_observation(false, true);
}

fn check_observation(undo: bool, stationary_state: bool) {
	let fixture = tempfile::tempdir().unwrap();
	let cwd = fixture.path().canonicalize().unwrap();
	let source = repo(&cwd, "api");
	let root = cwd.join("before");
	let destination = cwd.join("after");
	let state = cwd.join("state");
	let next_state = if stationary_state {
		state.clone()
	} else {
		cwd.join("new-state")
	};
	fs::create_dir(&root).unwrap();
	assert_ok(&workyard(
		&root,
		&[
			"create",
			"feat/one",
			"--repo",
			text(&source),
			"--from",
			"main",
			"--no-fetch",
			"--state-dir",
			text(&state),
		],
	));

	let mut session =
		Relocation::open(&state, Some(&root), &destination, Some(&next_state)).unwrap();
	session.begin(session.participants()).unwrap();
	drop(session);
	assert_observed(&cwd, &state, &root, false);

	let mut session = Relocation::recover(&state).unwrap();
	session.activate(undo).unwrap();
	drop(session);
	let active_root = if undo { &root } else { &destination };
	assert_observed(&cwd, &state, active_root, true);
	if state != next_state {
		assert_observed(&cwd, &next_state, active_root, true);
	}
}

fn assert_observed(cwd: &Path, state: &Path, root: &Path, complete: bool) {
	for command in ["list", "status", "info"] {
		let output = workyard(cwd, &[command, "--state-dir", text(state), "--json"]);
		assert_ok(&output);
		let response: Value = serde_json::from_slice(&output.stdout).unwrap();
		let result = &response["result"];
		assert_eq!(result["tasks_root"], json!(root), "{command}");
		assert_eq!(result["state_dir"], json!(state), "{command}");
		assert_eq!(result["relocation"]["placement_complete"], complete);

		let guidance = result["guidance"].as_str().unwrap();
		assert!(guidance.contains("workyard recover --state-dir="));
		assert_eq!(guidance.contains("cleanup"), complete);
		assert_eq!(guidance.contains("--undo"), !complete);
	}
}
