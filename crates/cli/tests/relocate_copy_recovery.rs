#[allow(dead_code)]
mod support;

use std::{
	fs,
	path::{Path, PathBuf},
};
use support::*;
use workyard_core::{Control, RelocationState};
use workyard_state::Relocation;

struct InterruptedCopy {
	fixture: tempfile::TempDir,
	source: PathBuf,
	target: PathBuf,
	api: PathBuf,
	web: PathBuf,
}

impl InterruptedCopy {
	/// Stop after Git rebound api but before Workyard recorded its success.
	fn after_first_repair() -> Self {
		let fixture = tempfile::tempdir().unwrap();
		let cwd = fixture.path().canonicalize().unwrap();
		let api = repo(&cwd, "api");
		let web = repo(&cwd, "web");
		let source = cwd.join("old");
		let target = cwd.join("new");
		fs::create_dir(&source).unwrap();
		assert_ok(&workyard(
			&source,
			&[
				"create",
				"feat/one",
				"--repo",
				text(&api),
				"--repo",
				text(&web),
				"--from",
				"main",
				"--no-fetch",
			],
		));
		let control = Control::default();
		let mut session = Relocation::open_copy_control(
			&source.join(".workyard"),
			Some(&source),
			&target,
			None,
			&control,
		)
		.unwrap();
		session.begin(session.participants()).unwrap();
		session.transfer(false, &control).unwrap();
		drop(session);
		git(
			&api,
			&[
				"worktree",
				"repair",
				"--",
				text(&target.join("feat/one/api")),
			],
		);
		Self {
			fixture,
			source,
			target,
			api,
			web,
		}
	}

	fn recover(&self, undo: bool) -> std::process::Output {
		let state = self.source.join(".workyard");
		let mut args = vec!["recover", "--state-dir", text(&state)];
		if undo {
			args.push("--undo");
		}
		workyard(self.fixture.path(), &args)
	}

	fn assert_registered(&self, root: &Path) {
		for (name, repo) in [("api", &self.api), ("web", &self.web)] {
			let registered = git(repo, &["worktree", "list", "--porcelain"]);
			let path = root.join("feat/one").join(name);
			assert!(
				registered.contains(&format!(
					"worktree {}",
					text(&path).replace('\\', "/").trim_start_matches("//?/")
				)),
				"{registered}"
			);
		}
	}
}

#[test]
fn forward_recovery_completes_partial_git_handoff() {
	let copy = InterruptedCopy::after_first_repair();
	assert_ok(&copy.recover(false));
	copy.assert_registered(&copy.target);
	assert!(copy.source.join("feat/one/api/file").is_file());
	assert_ok(&workyard(&copy.target, &["status", "feat/one"]));
	assert!(
		!workyard(&copy.source, &["remove", "feat/one", "--yes"])
			.status
			.success()
	);
	copy.assert_registered(&copy.target);
}

#[test]
fn undo_rebinds_source_and_retains_edits_in_partial_copy() {
	let copy = InterruptedCopy::after_first_repair();
	let edited = copy.target.join("feat/one/api/file");
	fs::write(&edited, "destination edit").unwrap();
	assert_ok(&copy.recover(true));
	copy.assert_registered(&copy.source);
	assert_eq!(fs::read_to_string(edited).unwrap(), "destination edit");
	assert_eq!(
		fs::read_to_string(copy.source.join("feat/one/api/file")).unwrap(),
		"initial\n"
	);
	assert_ok(&workyard(&copy.source, &["status", "feat/one"]));
	assert!(
		!workyard(&copy.target, &["remove", "feat/one", "--yes"])
			.status
			.success()
	);
}

#[path = "support/relocation/displaced.rs"]
mod displaced;

#[test]
fn undo_preserves_a_foreign_git_pointer_at_the_registered_destination() {
	let copy = InterruptedCopy::after_first_repair();
	let pointer = copy.target.join("feat/one/api/.git");
	let foreign = fs::read(copy.target.join("feat/one/web/.git")).unwrap();
	fs::write(&pointer, &foreign).unwrap();

	let refused = copy.recover(true);
	assert!(!refused.status.success());
	assert_eq!(fs::read(pointer).unwrap(), foreign);
}

#[test]
fn copy_repairs_relative_git_pointers_across_directory_depths() {
	let fixture = tempfile::tempdir().unwrap();
	let api = repo(fixture.path(), "api");
	let source = fixture.path().join("source");
	let target = fixture.path().join("nested/destination");
	fs::create_dir(&source).unwrap();
	assert_ok(&workyard(
		&source,
		&[
			"create",
			"feat/one",
			"--repo",
			text(&api),
			"--from",
			"main",
			"--no-fetch",
		],
	));
	let worktree = source.join("feat/one/api");
	git(
		&api,
		&["worktree", "repair", "--relative-paths", text(&worktree)],
	);
	let original_pointer = fs::read(worktree.join(".git")).unwrap();

	assert_ok(&workyard(
		&source,
		&[
			"relocate",
			"--old",
			text(&source),
			"--new",
			text(&target),
			"--keep-source",
		],
	));
	assert_eq!(fs::read(worktree.join(".git")).unwrap(), original_pointer);
	assert_eq!(
		git(&target.join("feat/one/api"), &["branch", "--show-current"]),
		"feat/one"
	);
}

#[test]
fn undo_allows_a_missing_destination_that_git_never_rebound() {
	let copy = InterruptedCopy::after_first_repair();
	fs::remove_dir_all(copy.target.join("feat/one/web")).unwrap();
	assert_ok(&copy.recover(true));
	copy.assert_registered(&copy.source);
}

#[test]
fn recovery_reports_an_unsettled_git_writer_with_its_participant() {
	let copy = InterruptedCopy::after_first_repair();
	let state = copy.source.join(".workyard");
	let mut session = Relocation::recover(&state).unwrap();
	let api = session
		.participants()
		.into_iter()
		.find(|entry| entry.original.name == "api")
		.unwrap();
	session.copy_writer(&api, false).unwrap();
	drop(session);

	let output = copy.recover(false);
	assert!(!output.status.success());
	let rendered = format!(
		"{}{}",
		String::from_utf8_lossy(&output.stdout),
		String::from_utf8_lossy(&output.stderr)
	);
	assert!(
		rendered.contains("resource_busy") && rendered.contains("api"),
		"{rendered}"
	);
}
