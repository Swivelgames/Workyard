//! Real Git regression cases for undo preflight, retained branches and renewed consent.
use super::*;
use std::sync::atomic::{AtomicBool, Ordering};
use workyard_core::*;

#[test]
fn initial_undo_refuses_missing_survivor_before_any_recreation() {
	let workspace = Workspace::new();
	interrupt_removal(&workspace, false);
	fs::remove_dir_all(workspace.tree("web")).unwrap();

	let before = durable_task_bytes(&workspace.root);

	let output = workyard(&workspace.root, &["recover", "feat/task", "--undo"]);
	assert!(!output.status.success());
	assert!(!workspace.tree("api").exists());
	assert_eq!(durable_task_bytes(&workspace.root), before);
}

struct StopAfterWeb;

impl Observer for StopAfterWeb {
	async fn phase(&self, _: &str, participant: &str, phase: &str) -> Result<(), Problem> {
		if participant == "web" && phase == "removed" {
			return Err(Problem::new(
				ProblemCode::Cancelled,
				"Stop before final retirement",
			));
		}
		Ok(())
	}
}

impl RemovalObserver for StopAfterWeb {
	async fn disclose(&self, _: &str, _: &RemovalObservation) -> Result<(), Problem> {
		Ok(())
	}

	async fn confirm(&self, _: Decision) -> Result<Option<bool>, Problem> {
		Ok(Some(true))
	}
}

#[test]
fn retry_records_the_branch_actually_removed_for_later_undo() {
	let workspace = Workspace::new();
	interrupt_removal(&workspace, false);
	git(&workspace.tree("web"), &["checkout", "-b", "feat/actual"]);
	fs::write(workspace.tree("web").join("file"), "actual branch work").unwrap();
	git(
		&workspace.tree("web"),
		&["commit", "-am", "new branch work"],
	);

	let expected = git(&workspace.tree("web"), &["rev-parse", "HEAD"]);

	let store =
		workyard_state::Store::open_existing(&workspace.root.join(".workyard"), None).unwrap();

	let removed = smol::block_on(recover_removal(
		"feat/task",
		RemovalOptions::default(),
		&workyard_git::Runner::default(),
		&Control::default(),
		&StopAfterWeb,
		&store,
	))
	.unwrap();
	assert_eq!(removed.operation.outcome, OperationOutcome::Cancelled);
	assert_eq!(
		removed.task.removal.unwrap().participants[1]
			.branch
			.as_deref(),
		Some("feat/actual")
	);
	drop(store);

	assert_ok(&workyard(
		&workspace.root,
		&["recover", "feat/task", "--undo"],
	));
	assert_eq!(
		git(&workspace.tree("web"), &["branch", "--show-current"]),
		"feat/actual"
	);
	assert_eq!(
		git(&workspace.tree("web"), &["rev-parse", "HEAD"]),
		expected
	);
}

struct ChangeDirty {
	tree: PathBuf,
	index_only: bool,
	changed: AtomicBool,
}

impl Observer for ChangeDirty {
	async fn phase(&self, _: &str, _: &str, _: &str) -> Result<(), Problem> {
		Ok(())
	}
}

impl RemovalObserver for ChangeDirty {
	async fn disclose(&self, participant: &str, _: &RemovalObservation) -> Result<(), Problem> {
		if participant != "api" || self.changed.swap(true, Ordering::SeqCst) {
			return Ok(());
		}
		fs::write(self.tree.join("file"), "new dirty").unwrap();
		if self.index_only {
			git(&self.tree, &["add", "file"]);
			fs::write(self.tree.join("file"), "work tree").unwrap();
		}
		Ok(())
	}

	async fn confirm(&self, decision: Decision) -> Result<Option<bool>, Problem> {
		assert_eq!(decision.kind, DecisionKind::DiscardFiles);
		Ok(Some(false))
	}
}

#[test]
fn changing_already_dirty_bytes_or_index_renews_consent_without_state_changes() {
	for index_only in [false, true] {
		let workspace = Workspace::new();
		let tree = workspace.tree("api");
		fs::write(tree.join("file"), "old dirty").unwrap();
		if index_only {
			git(&tree, &["add", "file"]);
			fs::write(tree.join("file"), "work tree").unwrap();
		}

		let status = git(&tree, &["status", "--porcelain"]);

		let before = durable_task_bytes(&workspace.root);
		let observer = ChangeDirty {
			tree: tree.clone(),
			index_only,
			changed: AtomicBool::new(false),
		};

		let store =
			workyard_state::Store::open_existing(&workspace.root.join(".workyard"), None).unwrap();

		let result = smol::block_on(remove(
			"feat/task",
			RemovalOptions {
				force: true,
				..Default::default()
			},
			&workyard_git::Runner::default(),
			&Control::default(),
			&observer,
			&store,
		))
		.unwrap();

		assert_eq!(result.operation.outcome, OperationOutcome::Cancelled);
		assert!(!result.mutation_started);
		assert_eq!(durable_task_bytes(&workspace.root), before);
		assert_eq!(git(&tree, &["status", "--porcelain"]), status);
		assert_eq!(
			fs::read_to_string(tree.join("file")).unwrap(),
			if index_only { "work tree" } else { "new dirty" }
		);
		if index_only {
			assert_eq!(git(&tree, &["show", ":file"]), "new dirty");
		}
		assert!(workspace.tree("web").exists());
	}
}
