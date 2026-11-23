mod support;
use std::{fs, path::PathBuf};
use support::*;

struct Workspace {
	_temp: tempfile::TempDir,
	root: PathBuf,
	api: PathBuf,
	web: PathBuf,
}

impl Workspace {
	fn new() -> Self {
		let temp = tempfile::tempdir().unwrap();
		let api = repo(temp.path(), "api");
		let web = repo(temp.path(), "web");
		let root = temp.path().join("workspace");
		init_workspace(&root);
		let workspace = Self {
			_temp: temp,
			root,
			api,
			web,
		};
		workspace.create();
		workspace
	}

	fn create(&self) {
		assert_ok(&workyard(
			&self.root,
			&[
				"checkout",
				"feat/task",
				"--repo",
				text(&self.api),
				"--repo",
				text(&self.web),
				"--from",
				"main",
				"--no-fetch",
			],
		));
	}

	fn tree(&self, participant: &str) -> PathBuf {
		self.root.join("tasks/feat/task").join(participant)
	}
}

#[test]
fn clean_removal_preserves_branches_retires_and_allows_a_new_incarnation() {
	let workspace = Workspace::new();
	let before = record(&workspace.root.join(".workyard"));
	let output = workyard(&workspace.root, &["remove", "feat/task"]);
	assert_ok(&output);
	assert!(String::from_utf8_lossy(&output.stderr).contains("no upstream"));
	assert!(!workspace.tree("api").exists());
	assert!(!workspace.tree("web").exists());
	git(
		&workspace.api,
		&["show-ref", "--verify", "refs/heads/feat/task"],
	);
	assert_eq!(
		record(&workspace.root.join(".workyard"))["lifecycle"],
		"retired"
	);
	let retired = workyard(&workspace.root, &["remove", "feat/task"]);
	assert_ok(&retired);
	assert_eq!(human_body(&retired), "  Already retired: feat/task\n");
	assert!(
		!workyard(&workspace.root, &["recover", "feat/task", "--undo"])
			.status
			.success()
	);

	workspace.create();
	let current = record(&workspace.root.join(".workyard"));
	assert!(current["generation"].as_u64().unwrap() > before["generation"].as_u64().unwrap());
	assert_eq!(current["lifecycle"], "active");
	let status = workyard(&workspace.root, &["list"]);
	assert_ok(&status);
	assert!(String::from_utf8_lossy(&status.stdout).contains("Recently closed tasks"));
}

#[test]
fn yes_never_discards_files_and_force_includes_ignored_files() {
	let workspace = Workspace::new();
	fs::write(workspace.tree("web").join("file"), "modified").unwrap();
	fs::write(workspace.tree("web").join("untracked"), "untracked").unwrap();
	fs::write(workspace.web.join(".git/info/exclude"), "ignored\n").unwrap();
	fs::write(workspace.tree("web").join("ignored"), "ignored").unwrap();
	let rejected = workyard(&workspace.root, &["remove", "feat/task", "--yes"]);
	assert!(!rejected.status.success());
	assert!(
		workspace.tree("api").exists(),
		"inspect all before removing any"
	);
	assert!(workspace.tree("web").join("ignored").exists());
	assert!(String::from_utf8_lossy(&rejected.stdout).contains("Removal needs confirmation"));
	assert_ok(&workyard(
		&workspace.root,
		&["remove", "feat/task", "--force"],
	));
}

#[test]
fn force_does_not_bypass_unfinished_git_or_unprotected_detached_history() {
	let workspace = Workspace::new();
	let tree = workspace.tree("web");
	let administrative = PathBuf::from(git(&tree, &["rev-parse", "--absolute-git-dir"]));
	fs::write(
		administrative.join("MERGE_HEAD"),
		git(&tree, &["rev-parse", "HEAD"]),
	)
	.unwrap();
	assert!(
		!workyard(&workspace.root, &["remove", "feat/task", "--force"])
			.status
			.success()
	);
	assert!(workspace.tree("api").exists());
	fs::remove_file(administrative.join("MERGE_HEAD")).unwrap();

	git(&tree, &["checkout", "--detach"]);
	fs::write(tree.join("file"), "detached commit").unwrap();
	git(&tree, &["commit", "-am", "detached"]);
	assert!(
		!workyard(&workspace.root, &["remove", "feat/task", "--force"])
			.status
			.success()
	);
	git(&tree, &["tag", "keep-detached"]);
	assert_ok(&workyard(&workspace.root, &["remove", "feat/task"]));
}

#[test]
fn unrelated_task_directory_files_are_preserved() {
	let workspace = Workspace::new();
	let unrelated = workspace.root.join("tasks/feat/task/notes");
	fs::write(&unrelated, "user notes").unwrap();
	assert_ok(&workyard(&workspace.root, &["remove", "feat/task"]));
	assert_eq!(fs::read_to_string(unrelated).unwrap(), "user notes");
	assert!(
		!workyard(
			&workspace.root,
			&["checkout", "feat/task", "--repo", text(&workspace.api)]
		)
		.status
		.success()
	);
}

struct CancelAfterFirst(std::sync::Arc<std::sync::atomic::AtomicBool>);

impl workyard_core::Observer for CancelAfterFirst {
	async fn phase(
		&self,
		_: &str,
		participant: &str,
		phase: &str,
	) -> Result<(), workyard_core::Problem> {
		if participant == "api" && phase == "removed" {
			self.0.store(true, std::sync::atomic::Ordering::SeqCst);
		}
		Ok(())
	}
}

impl workyard_core::RemovalObserver for CancelAfterFirst {
	async fn disclose(
		&self,
		_: &str,
		_: &workyard_core::RemovalObservation,
	) -> Result<(), workyard_core::Problem> {
		Ok(())
	}
	async fn confirm(
		&self,
		_: workyard_core::Decision,
	) -> Result<Option<bool>, workyard_core::Problem> {
		Ok(None)
	}
}

fn interrupt_removal(workspace: &Workspace, force: bool) {
	use workyard_core::{Control, RemovalOptions};
	let cancelled = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
	let signal = cancelled.clone();
	let control = Control::new(move || signal.load(std::sync::atomic::Ordering::SeqCst));
	let store =
		workyard_state::Store::open_existing(&workspace.root.join(".workyard"), None).unwrap();
	let result = smol::block_on(workyard_core::remove(
		"feat/task",
		RemovalOptions {
			force,
			..Default::default()
		},
		&workyard_git::Runner::default(),
		&control,
		&CancelAfterFirst(cancelled),
		&store,
	))
	.unwrap();
	assert_eq!(
		result.operation.outcome,
		workyard_core::OperationOutcome::Cancelled
	);
	assert!(!workspace.tree("api").exists());
	assert!(workspace.tree("web").exists());
}

#[test]
fn recovery_continues_partial_removal_without_replaying_successes() {
	let workspace = Workspace::new();
	interrupt_removal(&workspace, false);
	assert_eq!(
		record(&workspace.root.join(".workyard"))["lifecycle"],
		"active"
	);
	let status = workyard(&workspace.root, &["status", "feat/task"]);
	assert_ok(&status);
	let body = human_body(&status);
	let row = body
		.lines()
		.find(|line| line.trim_start().starts_with("api "))
		.unwrap();
	assert!(row.ends_with("removed"), "{body}");

	let locations = workyard(&workspace.root, &["recover"]);
	assert_ok(&locations);
	let body = human_body(&locations);
	assert!(
		body.contains("No workspace location repairs needed"),
		"{body}"
	);
	assert!(body.contains("workyard recover feat/task"), "{body}");

	let recovered = workyard(&workspace.root, &["recover", "feat/task"]);
	assert_ok(&recovered);
	assert_eq!(human_body(&recovered), "  Recovery succeeded: feat/task\n");
	let progress = String::from_utf8(recovered.stderr).unwrap();
	assert!(progress.contains("Recover feat/task\n"), "{progress}");
	assert!(progress.contains("    web: removed"), "{progress}");
	assert_eq!(
		record(&workspace.root.join(".workyard"))["lifecycle"],
		"retired"
	);
}

#[test]
fn undo_requires_lost_changes_acknowledgement_and_uses_retained_branch_tip() {
	let workspace = Workspace::new();
	fs::write(workspace.tree("api").join("lost"), "cannot restore").unwrap();
	interrupt_removal(&workspace, true);
	assert!(
		!workyard(&workspace.root, &["recover", "feat/task", "--undo"])
			.status
			.success()
	);
	assert!(!workspace.tree("api").exists());

	git(&workspace.api, &["checkout", "feat/task"]);
	fs::write(workspace.api.join("file"), "advanced branch").unwrap();
	git(
		&workspace.api,
		&["commit", "-am", "advance retained branch"],
	);
	let tip = git(&workspace.api, &["rev-parse", "HEAD"]);
	git(&workspace.api, &["checkout", "main"]);
	let restored = workyard(
		&workspace.root,
		&["recover", "feat/task", "--undo", "--yes"],
	);
	assert_ok(&restored);
	assert_eq!(human_body(&restored), "  Recovery succeeded: feat/task\n");
	let progress = String::from_utf8(restored.stderr).unwrap();
	assert!(progress.contains("    api: restored"), "{progress}");
	assert!(!progress.contains("recover ·"), "{progress}");
	assert_eq!(git(&workspace.tree("api"), &["rev-parse", "HEAD"]), tip);
	assert!(!workspace.tree("api").join("lost").exists());
	// Completed restoration stays a no-op even when later authored work is detached.
	git(&workspace.tree("api"), &["checkout", "--detach"]);
	fs::write(workspace.tree("api").join("file"), "later authored work").unwrap();
	git(
		&workspace.tree("api"),
		&["commit", "-am", "detached authored work"],
	);
	let before = durable_task_bytes(&workspace.root);
	for arguments in [
		vec!["recover", "feat/task", "--undo", "--yes"],
		vec!["recover", "feat/task", "--yes"],
	] {
		let repeated = workyard(&workspace.root, &arguments);
		assert_ok(&repeated);
		assert_eq!(human_body(&repeated), "  Nothing to recover: feat/task\n");
		assert_eq!(durable_task_bytes(&workspace.root), before);
	}
	assert!(workspace.tree("web").exists());
	fs::remove_dir_all(workspace.tree("web")).unwrap();
	let missing = workyard(&workspace.root, &["recover", "feat/task", "--yes"]);
	assert!(!missing.status.success());
	assert!(!String::from_utf8_lossy(&missing.stdout).contains("Nothing to recover"));
	assert_eq!(durable_task_bytes(&workspace.root), before);
}

#[test]
fn undo_never_overwrites_a_destination_or_displaces_an_existing_worktree() {
	let workspace = Workspace::new();
	interrupt_removal(&workspace, false);
	fs::create_dir(workspace.tree("api")).unwrap();
	fs::write(workspace.tree("api").join("keep"), "keep").unwrap();
	assert!(
		!workyard(
			&workspace.root,
			&["recover", "feat/task", "--undo", "--force"]
		)
		.status
		.success()
	);
	assert_eq!(
		fs::read_to_string(workspace.tree("api").join("keep")).unwrap(),
		"keep"
	);
	fs::remove_dir_all(workspace.tree("api")).unwrap();
	git(&workspace.api, &["checkout", "feat/task"]);
	assert!(
		!workyard(
			&workspace.root,
			&["recover", "feat/task", "--undo", "--force"]
		)
		.status
		.success()
	);
	assert!(workspace.tree("web").exists());
}

#[test]
fn clean_unpushed_branch_is_disclosed_without_noninteractive_confirmation() {
	let fixture = tempfile::tempdir().unwrap();
	let (api, _remote) = remote_clone(fixture.path(), "api");
	git(&api, &["config", "user.email", "fixture@example.invalid"]);
	git(&api, &["config", "user.name", "Fixture"]);
	let root = fixture.path().join("workspace");
	init_workspace(&root);
	assert_ok(&workyard(
		&root,
		&[
			"create",
			"feat/task",
			"--repo",
			text(&api),
			"--from",
			"main",
			"--no-fetch",
		],
	));
	let tree = root.join("tasks/feat/task/api");
	git(&tree, &["branch", "--set-upstream-to=origin/main"]);
	fs::write(tree.join("file"), "unpushed").unwrap();
	git(&tree, &["commit", "-am", "local commit"]);
	let commit = git(&tree, &["rev-parse", "HEAD"]);
	let output = workyard(&root, &["remove", "feat/task"]);
	assert_ok(&output);
	assert!(String::from_utf8_lossy(&output.stderr).contains("unpushed"));
	assert_eq!(git(&api, &["rev-parse", "feat/task"]), commit);
}

fn durable_task_bytes(root: &std::path::Path) -> Vec<(PathBuf, Vec<u8>)> {
	let mut documents = Vec::new();
	for directory in ["tasks", "evidence"] {
		for entry in fs::read_dir(root.join(".workyard").join(directory)).unwrap() {
			let path = entry.unwrap().path();
			documents.push((path.clone(), fs::read(path).unwrap()));
		}
	}
	documents.sort();
	documents
}

struct DeclineTask;

impl workyard_core::Observer for DeclineTask {
	async fn phase(&self, _: &str, _: &str, _: &str) -> Result<(), workyard_core::Problem> {
		Ok(())
	}
}

impl workyard_core::RemovalObserver for DeclineTask {
	async fn disclose(
		&self,
		_: &str,
		_: &workyard_core::RemovalObservation,
	) -> Result<(), workyard_core::Problem> {
		Ok(())
	}
	async fn confirm(
		&self,
		decision: workyard_core::Decision,
	) -> Result<Option<bool>, workyard_core::Problem> {
		assert!(decision.task.is_some());
		Ok(Some(false))
	}
}

#[test]
fn task_decline_keeps_task_history_identical_and_retry_works() {
	let workspace = Workspace::new();
	let before = durable_task_bytes(&workspace.root);
	let store =
		workyard_state::Store::open_existing(&workspace.root.join(".workyard"), None).unwrap();
	let result = smol::block_on(workyard_core::remove(
		"feat/task",
		Default::default(),
		&workyard_git::Runner::default(),
		&workyard_core::Control::new(|| false),
		&DeclineTask,
		&store,
	))
	.unwrap();
	drop(store);

	assert_eq!(
		result.operation.outcome,
		workyard_core::OperationOutcome::Cancelled
	);
	assert_eq!(
		result.task.operation.outcome,
		workyard_core::OperationOutcome::Succeeded
	);
	assert!(!result.mutation_started);
	assert!(!result.needs_attention);
	assert!(result.inspected);
	assert_eq!(durable_task_bytes(&workspace.root), before);
	assert!(workspace.tree("api").exists());
	assert!(workspace.tree("web").exists());
	let status = workyard(&workspace.root, &["status", "feat/task"]);
	assert_ok(&status);
	assert!(String::from_utf8_lossy(&status.stdout).contains("[healthy]"));
	assert_ok(&workyard(
		&workspace.root,
		&["remove", "feat/task", "--yes"],
	));
}

#[test]
fn noninteractive_decision_requirement_does_not_become_task_failure() {
	let workspace = Workspace::new();
	fs::write(workspace.tree("web").join("file"), "user changes").unwrap();
	let before = durable_task_bytes(&workspace.root);
	let output = workyard(&workspace.root, &["remove", "feat/task", "--yes"]);

	assert!(!output.status.success());
	let text = String::from_utf8(output.stdout).unwrap();
	assert!(text.contains("Removal needs confirmation"), "{text}");
	assert!(text.contains("No worktrees changed."), "{text}");
	assert!(!text.contains("needs attention"), "{text}");
	assert!(!text.contains("removal unconfirmed"), "{text}");
	assert_eq!(durable_task_bytes(&workspace.root), before);
	assert_eq!(
		fs::read_to_string(workspace.tree("web").join("file")).unwrap(),
		"user changes"
	);
	assert_ok(&workyard(
		&workspace.root,
		&["remove", "feat/task", "--force"],
	));
}

#[test]
fn a_decline_preserves_preexisting_diagnostics_instead_of_claiming_health() {
	let workspace = Workspace::new();
	let state = workspace.root.join(".workyard");
	let path = state
		.join("tasks")
		.join(workyard_state::task_filename("feat/task"));
	let mut record: serde_json::Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
	record["operation"]["outcome"] = "failed".into();
	record["problem"] = serde_json::json!({
		"code": "publication_uncertain",
		"message": "Prior publication needs review",
		"remedy": "Inspect prior state",
	});
	fs::write(&path, serde_json::to_vec(&record).unwrap()).unwrap();
	let before = durable_task_bytes(&workspace.root);
	let store = workyard_state::Store::open_existing(&state, None).unwrap();
	let result = smol::block_on(workyard_core::remove(
		"feat/task",
		Default::default(),
		&workyard_git::Runner::default(),
		&workyard_core::Control::new(|| false),
		&DeclineTask,
		&store,
	))
	.unwrap();

	assert!(!result.mutation_started);
	assert!(result.needs_attention);
	assert_eq!(
		result.task.problem.unwrap().code,
		workyard_core::ProblemCode::PublicationUncertain
	);
	assert_eq!(durable_task_bytes(&workspace.root), before);
}

struct ChangeAfterDisclosure {
	file: PathBuf,
	changed: std::sync::atomic::AtomicBool,
}

impl workyard_core::Observer for ChangeAfterDisclosure {
	async fn phase(&self, _: &str, _: &str, _: &str) -> Result<(), workyard_core::Problem> {
		Ok(())
	}
}

impl workyard_core::RemovalObserver for ChangeAfterDisclosure {
	async fn disclose(
		&self,
		participant: &str,
		_: &workyard_core::RemovalObservation,
	) -> Result<(), workyard_core::Problem> {
		if participant == "api" && !self.changed.swap(true, std::sync::atomic::Ordering::SeqCst) {
			fs::write(&self.file, "new undisclosed changes").unwrap();
		}
		Ok(())
	}

	async fn confirm(
		&self,
		decision: workyard_core::Decision,
	) -> Result<Option<bool>, workyard_core::Problem> {
		assert_eq!(decision.kind, workyard_core::DecisionKind::DiscardFiles);
		Ok(Some(false))
	}
}

#[test]
fn declining_fresh_changes_before_the_first_effect_does_not_publish_removal() {
	let workspace = Workspace::new();
	let before = durable_task_bytes(&workspace.root);
	let observer = ChangeAfterDisclosure {
		file: workspace.tree("api").join("file"),
		changed: std::sync::atomic::AtomicBool::new(false),
	};
	let store =
		workyard_state::Store::open_existing(&workspace.root.join(".workyard"), None).unwrap();
	let result = smol::block_on(workyard_core::remove(
		"feat/task",
		workyard_core::RemovalOptions {
			force: true,
			..Default::default()
		},
		&workyard_git::Runner::default(),
		&workyard_core::Control::new(|| false),
		&observer,
		&store,
	))
	.unwrap();

	assert_eq!(
		result.operation.outcome,
		workyard_core::OperationOutcome::Cancelled
	);
	assert!(!result.mutation_started);
	assert_eq!(durable_task_bytes(&workspace.root), before);
	assert_eq!(
		fs::read_to_string(observer.file).unwrap(),
		"new undisclosed changes"
	);
	assert!(workspace.tree("web").exists());
}

#[test]
fn declining_undo_preserves_partial_effects_and_the_existing_recovery_requirement() {
	let workspace = Workspace::new();
	fs::write(workspace.tree("api").join("lost"), "cannot restore").unwrap();
	interrupt_removal(&workspace, true);
	let before = durable_task_bytes(&workspace.root);
	let output = workyard(&workspace.root, &["recover", "feat/task", "--undo"]);

	assert!(!output.status.success());
	let output = String::from_utf8_lossy(&output.stdout);
	assert!(output.contains("No worktrees changed."), "{output}");
	assert!(output.contains("Inspect: workyard status"), "{output}");
	assert!(!output.contains("Repository"), "{output}");
	assert_eq!(durable_task_bytes(&workspace.root), before);
	assert!(!workspace.tree("api").exists());
	assert!(workspace.tree("web").exists());
	assert_ok(&workyard(
		&workspace.root,
		&["recover", "feat/task", "--yes"],
	));
}

#[derive(Default)]
struct ConfirmTask {
	disclosed: std::sync::Mutex<Vec<String>>,
	questions: std::sync::Mutex<Vec<workyard_core::DecisionKind>>,
}

impl workyard_core::Observer for ConfirmTask {
	async fn phase(&self, _: &str, _: &str, _: &str) -> Result<(), workyard_core::Problem> {
		Ok(())
	}
}

impl workyard_core::RemovalObserver for ConfirmTask {
	async fn disclose(
		&self,
		participant: &str,
		_: &workyard_core::RemovalObservation,
	) -> Result<(), workyard_core::Problem> {
		self.disclosed.lock().unwrap().push(participant.into());
		Ok(())
	}

	async fn confirm(
		&self,
		decision: workyard_core::Decision,
	) -> Result<Option<bool>, workyard_core::Problem> {
		assert_eq!(*self.disclosed.lock().unwrap(), ["api", "web"]);
		assert_eq!(decision.task.as_deref(), Some("feat/task"));
		assert!(decision.participant.is_empty());
		self.questions.lock().unwrap().push(decision.kind);
		Ok(Some(true))
	}
}

#[test]
fn one_task_confirmation_covers_all_initial_disclosures_and_honors_flags() {
	use workyard_core::{DecisionKind, RemovalOptions};
	for (dirty, yes, force, expected) in [
		(false, false, false, Some(DecisionKind::RemoveTask)),
		(true, false, false, Some(DecisionKind::DiscardTask)),
		(true, true, false, Some(DecisionKind::DiscardTask)),
		(false, true, false, None),
		(true, false, true, None),
	] {
		let workspace = Workspace::new();
		if dirty {
			fs::write(workspace.tree("web").join("file"), "dirty").unwrap();
		}
		let store =
			workyard_state::Store::open_existing(&workspace.root.join(".workyard"), None).unwrap();
		let observer = ConfirmTask::default();
		let result = smol::block_on(workyard_core::remove(
			"feat/task",
			RemovalOptions {
				yes,
				force,
				undo: false,
			},
			&workyard_git::Runner::default(),
			&workyard_core::Control::default(),
			&observer,
			&store,
		))
		.unwrap();

		assert_eq!(
			*observer.questions.lock().unwrap(),
			expected.into_iter().collect::<Vec<_>>()
		);
		assert_eq!(result.task.lifecycle, workyard_core::TaskLifecycle::Retired);
		assert!(!workspace.tree("api").exists());
		assert!(!workspace.tree("web").exists());
	}
}

struct ChangeLaterParticipant {
	file: PathBuf,
	questions: std::sync::atomic::AtomicUsize,
}

impl workyard_core::Observer for ChangeLaterParticipant {
	async fn phase(
		&self,
		_: &str,
		participant: &str,
		phase: &str,
	) -> Result<(), workyard_core::Problem> {
		if participant == "api" && phase == "removed" {
			fs::write(&self.file, "new changes after consent").unwrap();
		}
		Ok(())
	}
}

impl workyard_core::RemovalObserver for ChangeLaterParticipant {
	async fn disclose(
		&self,
		_: &str,
		_: &workyard_core::RemovalObservation,
	) -> Result<(), workyard_core::Problem> {
		Ok(())
	}

	async fn confirm(
		&self,
		decision: workyard_core::Decision,
	) -> Result<Option<bool>, workyard_core::Problem> {
		let question = self
			.questions
			.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
		if question == 0 {
			assert_eq!(decision.kind, workyard_core::DecisionKind::RemoveTask);
			return Ok(Some(true));
		}
		assert_eq!(decision.kind, workyard_core::DecisionKind::DiscardFiles);
		assert_eq!(decision.participant, "web");
		Ok(Some(false))
	}
}

#[test]
fn declining_new_files_after_partial_removal_preserves_success_and_remaining_work() {
	let workspace = Workspace::new();
	let observer = ChangeLaterParticipant {
		file: workspace.tree("web").join("file"),
		questions: Default::default(),
	};
	let store =
		workyard_state::Store::open_existing(&workspace.root.join(".workyard"), None).unwrap();
	let result = smol::block_on(workyard_core::remove(
		"feat/task",
		Default::default(),
		&workyard_git::Runner::default(),
		&workyard_core::Control::default(),
		&observer,
		&store,
	))
	.unwrap();

	assert_eq!(
		observer.questions.load(std::sync::atomic::Ordering::SeqCst),
		2
	);
	assert!(result.mutation_started);
	assert_eq!(
		result.operation.outcome,
		workyard_core::OperationOutcome::Cancelled
	);
	assert!(!workspace.tree("api").exists());
	assert_eq!(
		fs::read_to_string(observer.file).unwrap(),
		"new changes after consent"
	);
	let saved = record(&workspace.root.join(".workyard"));
	assert_eq!(saved["removal"]["participants"][0]["phase"], "removed");
	assert_eq!(saved["removal"]["participants"][1]["phase"], "present");
}

#[path = "support/removal_failures.rs"]
mod removal_failures;

#[path = "support/removal_regressions.rs"]
mod removal_regressions;
