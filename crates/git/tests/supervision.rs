use std::{
	fs,
	io::Write,
	process::{Command, Stdio},
	sync::{
		Arc,
		atomic::{AtomicBool, Ordering},
	},
	time::Duration,
};
use workyard_core::{Control, ProblemCode};
use workyard_git::Runner;

#[test]
#[allow(clippy::zombie_processes)] // Deliberately exercises a parent exiting before its child.
fn fixture() {
	let Ok(mode) = fs::read_to_string("fixture-mode") else {
		return;
	};
	if mode == "pressure" {
		for _ in 0..512 {
			std::io::stdout().write_all(&[b'o'; 8192]).unwrap();
			std::io::stderr().write_all(&[b'e'; 8192]).unwrap();
		}
		return;
	}
	Command::new(std::env::current_exe().unwrap())
		.args(["--exact", "descendant", "--nocapture"])
		.stdin(Stdio::null())
		.stdout(Stdio::null())
		.stderr(Stdio::null())
		.spawn()
		.unwrap();
	// Return before the descendant, even though it holds no pipe handles.
}

#[test]
fn descendant() {
	let Ok(mode) = fs::read_to_string("fixture-mode") else {
		return;
	};
	fs::write("ready", b"ready").unwrap();
	std::thread::sleep(Duration::from_millis(if mode == "cancel" {
		1500
	} else {
		150
	}));
	fs::write("descendant-complete", b"done").unwrap();
}

#[test]
fn drains_pressure_with_bounded_capture() {
	let directory = tempfile::tempdir().unwrap();
	fs::write(directory.path().join("fixture-mode"), "pressure").unwrap();
	let output = smol::block_on(Runner::new(std::env::current_exe().unwrap()).run(
		directory.path(),
		&["--exact".into(), "fixture".into(), "--nocapture".into()],
		&Control::default(),
	))
	.unwrap();
	assert!(output.status.success());
	assert!(output.truncated);
	assert_eq!(output.stdout.len(), 1024 * 1024);
	assert_eq!(output.stderr.len(), 1024 * 1024);
}

#[test]
fn waits_for_descendant_after_parent_and_pipes_exit() {
	let directory = tempfile::tempdir().unwrap();
	fs::write(directory.path().join("fixture-mode"), "descendant").unwrap();
	let output = smol::block_on(Runner::new(std::env::current_exe().unwrap()).run(
		directory.path(),
		&["--exact".into(), "fixture".into(), "--nocapture".into()],
		&Control::default(),
	))
	.unwrap();
	assert!(output.status.success());
	assert!(directory.path().join("descendant-complete").exists());
}

#[test]
fn cancellation_stops_descendant_even_after_parent_exit() {
	let directory = tempfile::tempdir().unwrap();
	fs::write(directory.path().join("fixture-mode"), "cancel").unwrap();
	let cancelled = Arc::new(AtomicBool::new(false));
	let observer = cancelled.clone();
	let control = Control::new(move || observer.load(Ordering::SeqCst));
	let result = std::thread::scope(|scope| {
		scope.spawn(|| {
			while !directory.path().join("ready").exists() {
				std::thread::sleep(Duration::from_millis(5));
			}
			cancelled.store(true, Ordering::SeqCst);
		});
		smol::block_on(Runner::new(std::env::current_exe().unwrap()).run(
			directory.path(),
			&["--exact".into(), "fixture".into(), "--nocapture".into()],
			&control,
		))
	});
	match result {
		Ok(output) => assert!(output.interrupted && output.status.success()),
		Err(problem) => assert_eq!(problem.code, ProblemCode::Cancelled),
	}
	assert!(!directory.path().join("descendant-complete").exists());
}

#[test]
fn failed_launch_has_no_child_effects() {
	let directory = tempfile::tempdir().unwrap();
	let result = smol::block_on(
		Runner::new(directory.path().join("missing-executable")).run(
			directory.path(),
			&[],
			&Control::default(),
		),
	);
	assert_eq!(result.err().unwrap().code, ProblemCode::IoError);
	assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 0);
}
