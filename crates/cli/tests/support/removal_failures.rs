//! Fault injection at post-add boundaries; Git effects still use the real runner.
#[path = "removal_failures/publication.rs"]
mod publication;
#[path = "removal_failures/test_git.rs"]
mod test_git;

use super::*;
use publication::{Boundary, FaultState};
use std::sync::atomic::{AtomicBool, Ordering};
use test_git::FaultGit;
use workyard_core::*;

struct Quiet;

impl Observer for Quiet {
	async fn phase(&self, _: &str, _: &str, _: &str) -> Result<(), Problem> {
		Ok(())
	}
}

impl RemovalObserver for Quiet {
	async fn disclose(&self, _: &str, _: &RemovalObservation) -> Result<(), Problem> {
		Ok(())
	}

	async fn confirm(&self, _: Decision) -> Result<Option<bool>, Problem> {
		Ok(Some(true))
	}
}

fn undo(workspace: &Workspace, git: &impl RemovalGit) -> RemovalResult {
	let store =
		workyard_state::Store::open_existing(&workspace.root.join(".workyard"), None).unwrap();
	smol::block_on(recover_removal(
		"feat/task",
		RemovalOptions {
			undo: true,
			yes: true,
			force: false,
		},
		git,
		&Control::default(),
		&Quiet,
		&store,
	))
	.unwrap()
}

#[test]
fn settled_success_after_identity_failure_reconciles_without_another_add() {
	let workspace = Workspace::new();
	interrupt_removal(&workspace, false);
	let git = FaultGit {
		identity_failure: AtomicBool::new(true),
		..Default::default()
	};

	let failed = undo(&workspace, &git);
	assert_eq!(failed.operation.outcome, OperationOutcome::Failed);
	assert_eq!(
		failed.task.removal.unwrap().participants[0].phase,
		RemovalPhase::Recreated
	);

	let authored = workspace.tree("api").join("authored");
	fs::write(&authored, "preserve after failed identity read").unwrap();

	let effects = workspace
		.root
		.join(".workyard/evidence")
		.join(workyard_state::task_filename("feat/task"))
		.with_extension("effects");

	let settled = fs::read(&effects).unwrap();
	let mut pending: serde_json::Value = serde_json::from_slice(&settled).unwrap();
	pending["pending"] = 0.into();
	fs::write(&effects, serde_json::to_vec(&pending).unwrap()).unwrap();

	let before = durable_task_bytes(&workspace.root);

	let blocked = workyard(
		&workspace.root,
		&["recover", "feat/task", "--undo", "--yes"],
	);
	assert!(!blocked.status.success());
	assert_eq!(durable_task_bytes(&workspace.root), before);
	fs::write(effects, settled).unwrap();

	let recovered = undo(&workspace, &git);
	assert_eq!(recovered.operation.outcome, OperationOutcome::Succeeded);
	assert_eq!(git.adds.load(Ordering::SeqCst), 1);
	assert_eq!(
		fs::read_to_string(authored).unwrap(),
		"preserve after failed identity read"
	);
	assert!(recovered.task.participants[0].worktree_identity.is_some());
}

#[test]
fn failed_settled_add_cannot_adopt_an_unrelated_registered_worktree() {
	let workspace = Workspace::new();
	interrupt_removal(&workspace, false);
	let failing = FaultGit {
		add_failure: true,
		..Default::default()
	};

	let failed = undo(&workspace, &failing);
	assert_eq!(
		failed.task.removal.unwrap().participants[0].phase,
		RemovalPhase::Restoring
	);
	git(
		&workspace.api,
		&[
			"worktree",
			"add",
			"-b",
			"unrelated",
			text(&workspace.tree("api")),
		],
	);

	let authored = workspace.tree("api").join("authored");
	fs::write(&authored, "unrelated work").unwrap();

	let runner = FaultGit::default();

	let refused = undo(&workspace, &runner);
	assert_eq!(refused.operation.outcome, OperationOutcome::Failed);
	assert_eq!(runner.adds.load(Ordering::SeqCst), 0);
	assert_eq!(
		git(&workspace.tree("api"), &["branch", "--show-current"]),
		"unrelated"
	);
	assert_eq!(fs::read_to_string(authored).unwrap(), "unrelated work");
}

#[test]
fn final_publication_failure_keeps_identity_and_never_replays_recreation() {
	for boundary in [Boundary::Task, Boundary::FinalEffect] {
		let workspace = Workspace::new();
		interrupt_removal(&workspace, false);
		let git = FaultGit::default();

		let store = FaultState {
			boundary,
			store: workyard_state::Store::open_existing(&workspace.root.join(".workyard"), None)
				.unwrap(),
			fail: AtomicBool::new(true),
		};

		let failed = smol::block_on(recover_removal(
			"feat/task",
			RemovalOptions {
				undo: true,
				yes: true,
				force: false,
			},
			&git,
			&Control::default(),
			&Quiet,
			&store,
		))
		.unwrap();
		assert_eq!(failed.operation.outcome, OperationOutcome::Failed);
		assert!(failed.task.participants[0].worktree_identity.is_some());
		drop(store);

		let authored = workspace.tree("api").join("authored");
		fs::write(&authored, "kept after publication failure").unwrap();

		let recovered = undo(&workspace, &git);
		assert_eq!(recovered.operation.outcome, OperationOutcome::Succeeded);
		assert_eq!(git.adds.load(Ordering::SeqCst), 1);
		assert_eq!(
			fs::read_to_string(authored).unwrap(),
			"kept after publication failure"
		);
	}
}

#[test]
fn successful_receipt_without_identity_cannot_adopt_a_different_location_or_branch() {
	for move_location in [false, true] {
		let workspace = Workspace::new();
		interrupt_removal(&workspace, false);

		let runner = FaultGit {
			identity_failure: AtomicBool::new(true),
			..Default::default()
		};
		assert_eq!(
			undo(&workspace, &runner).operation.outcome,
			OperationOutcome::Failed
		);
		let original = workspace.tree("api");
		let current = if move_location {
			let moved = workspace.root.join("unrelated-location");
			git(
				&workspace.api,
				&["worktree", "move", text(&original), text(&moved)],
			);
			moved
		} else {
			git(&original, &["checkout", "-b", "unrelated-branch"]);
			original
		};
		fs::write(current.join("authored"), "preserve this work").unwrap();

		let result = undo(&workspace, &runner);
		assert_eq!(result.operation.outcome, OperationOutcome::Failed);
		assert_eq!(runner.adds.load(Ordering::SeqCst), 1);
		assert_eq!(
			fs::read_to_string(current.join("authored")).unwrap(),
			"preserve this work"
		);
	}
}

#[test]
fn known_recreated_identity_rejects_a_replacement_registration() {
	let workspace = Workspace::new();
	interrupt_removal(&workspace, false);

	let runner = FaultGit::default();

	let store = FaultState {
		boundary: Boundary::FinalEffect,
		store: workyard_state::Store::open_existing(&workspace.root.join(".workyard"), None)
			.unwrap(),
		fail: AtomicBool::new(true),
	};

	let failed = smol::block_on(recover_removal(
		"feat/task",
		RemovalOptions {
			undo: true,
			yes: true,
			force: false,
		},
		&runner,
		&Control::default(),
		&Quiet,
		&store,
	))
	.unwrap();
	assert_eq!(failed.operation.outcome, OperationOutcome::Failed);
	drop(store);

	let tree = workspace.tree("api");
	let administration = PathBuf::from(git(&tree, &["rev-parse", "--absolute-git-dir"]));
	// Keep the original identity allocated while replacing the registration.
	fs::rename(administration, workspace.root.join("held-administration")).unwrap();
	fs::remove_dir_all(&tree).unwrap();
	git(
		&workspace.api,
		&["worktree", "add", text(&tree), "feat/task"],
	);
	fs::write(tree.join("authored"), "replacement work").unwrap();

	for arguments in [
		vec!["remove", "feat/task", "--yes"],
		vec!["recover", "feat/task", "--undo", "--yes"],
	] {
		let before = durable_task_bytes(&workspace.root);
		let result = workyard(&workspace.root, &arguments);

		assert!(!result.status.success());
		assert_eq!(durable_task_bytes(&workspace.root), before);
		assert_eq!(
			fs::read_to_string(tree.join("authored")).unwrap(),
			"replacement work"
		);
	}
	assert_eq!(runner.adds.load(Ordering::SeqCst), 1);
}

#[test]
fn failed_success_receipt_keeps_pending_writer_guard_instead_of_adopting_tree() {
	let workspace = Workspace::new();
	interrupt_removal(&workspace, false);

	let runner = FaultGit::default();

	let store = FaultState {
		boundary: Boundary::SuccessReceipt,
		store: workyard_state::Store::open_existing(&workspace.root.join(".workyard"), None)
			.unwrap(),
		fail: AtomicBool::new(true),
	};

	let failed = smol::block_on(recover_removal(
		"feat/task",
		RemovalOptions {
			undo: true,
			yes: true,
			force: false,
		},
		&runner,
		&Control::default(),
		&Quiet,
		&store,
	))
	.unwrap();
	assert_eq!(failed.operation.outcome, OperationOutcome::Failed);
	drop(store);

	let before = durable_task_bytes(&workspace.root);

	let result = workyard(
		&workspace.root,
		&["recover", "feat/task", "--undo", "--yes"],
	);
	assert!(!result.status.success());
	assert!(String::from_utf8_lossy(&result.stdout).contains("participant \"api\""));
	assert_eq!(durable_task_bytes(&workspace.root), before);
	assert!(workspace.tree("api").exists());
	assert_eq!(runner.adds.load(Ordering::SeqCst), 1);
}

#[test]
fn only_an_unsupervised_destructive_unlink_retains_writer_evidence() {
	use test_git::UnlinkBoundary::{Validate, Writer};
	for (boundary, expect_writer) in [(Validate, false), (Writer, true)] {
		let workspace = Workspace::new();
		let store =
			workyard_state::Store::open_existing(&workspace.root.join(".workyard"), None).unwrap();
		let git = FaultGit {
			fail_unlink: Some(boundary),
			..Default::default()
		};
		let request = AmendRequest {
			task: "feat/task".into(),
			repositories: Vec::new(),
			remove: vec!["web".into()],
			options: CreationOptions {
				checkout: false,
				no_fetch: true,
				pull: false,
			},
			removal_options: RemovalOptions {
				yes: true,
				..Default::default()
			},
			remote: None,
			from: None,
		};
		let result =
			smol::block_on(amend(request, &git, &Control::default(), &Quiet, &store)).unwrap();
		assert_eq!(result.problem.unwrap().code, ProblemCode::SupervisionFailed);
		assert_eq!(result.mutation_started, expect_writer);
		let guard = store.admit("feat/task").unwrap();
		let saved = store.load(&guard, "feat/task").unwrap();
		assert_eq!(
			saved
				.pending
				.map(|i| saved.task.participants[i].name.as_str()),
			expect_writer.then_some("web")
		);
		assert_eq!(
			guard.evidence_path.with_extension("revision").exists(),
			expect_writer
		);
		if expect_writer {
			drop(guard);
			let refused = workyard(&workspace.root, &["recover", "feat/task", "--force"]);
			assert!(!refused.status.success());
			assert!(workspace.tree("web").join(".git").exists());
		}
	}
}
