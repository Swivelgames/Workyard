use super::{create, state_bytes};
use crate::support::{assert_ok, git, repo, text, workyard};
use std::path::Path;

struct DisruptAddition {
	source: std::path::PathBuf,
}

impl workyard_core::RemovalObserver for DisruptAddition {}

impl workyard_core::Observer for DisruptAddition {
	async fn phase(
		&self,
		_task: &str,
		_participant: &str,
		phase: &str,
	) -> Result<(), workyard_core::Problem> {
		if phase == "renamed" {
			git(&self.source, &["branch", "feat/task"]);
		}
		Ok(())
	}
}

fn request(repositories: Vec<workyard_core::RepositoryInput>) -> workyard_core::AmendRequest {
	workyard_core::AmendRequest {
		task: "feat/task".into(),
		repositories,
		remove: Vec::new(),
		removal_options: Default::default(),
		options: workyard_core::CreationOptions {
			checkout: false,
			no_fetch: true,
		},
		remote: None,
		from: Some("main".into()),
	}
}

fn repository(path: &Path, name: Option<&str>) -> workyard_core::RepositoryInput {
	workyard_core::RepositoryInput {
		registration_roots: Vec::new(),
		expected_identity: None,
		path: path.into(),
		name: name.map(str::to_owned),
		remote: None,
		from: None,
	}
}

#[test]
fn successful_rename_survives_later_failure_and_same_request_can_retry() {
	let fixture = tempfile::tempdir().unwrap();
	let api = repo(fixture.path(), "api");
	let web = repo(fixture.path(), "replacement");
	let root = fixture.path().join("workspace");
	create(&root, &api);
	{
		let store = workyard_state::Store::open_existing(&root.join(".workyard"), None).unwrap();
		let result = smol::block_on(workyard_core::amend(
			request(vec![
				repository(&api, Some("legacy")),
				repository(&web, Some("api")),
			]),
			&workyard_git::Runner::default(),
			&workyard_core::Control::new(|| false),
			&DisruptAddition {
				source: web.clone(),
			},
			&store,
		))
		.unwrap();
		assert_eq!(
			result.operation.outcome,
			workyard_core::OperationOutcome::Failed
		);
		assert_eq!(result.task.participants.len(), 1);
		assert_eq!(result.task.participants[0].name, "legacy");
	}
	let status = workyard(&root, &["status"]);
	assert_ok(&status);
	assert!(
		String::from_utf8_lossy(&status.stdout).contains("healthy"),
		"{}",
		String::from_utf8_lossy(&status.stdout)
	);
	git(&web, &["branch", "-D", "feat/task"]);
	assert_ok(&workyard(
		&root,
		&[
			"amend",
			"feat/task",
			"--from",
			"main",
			"--no-fetch",
			"--repo",
			text(&api),
			"--repo-name",
			"legacy",
			"--repo",
			text(&web),
			"--repo-name",
			"api",
		],
	));
	assert!(root.join("tasks/feat/task/legacy/.git").is_file());
	assert!(root.join("tasks/feat/task/api/.git").is_file());
}

struct CancelCreation;
impl workyard_core::RemovalObserver for CancelCreation {}

impl workyard_core::Observer for CancelCreation {
	async fn phase(
		&self,
		_task: &str,
		_participant: &str,
		phase: &str,
	) -> Result<(), workyard_core::Problem> {
		if phase == "creating" {
			return Err(workyard_core::Problem::new(
				workyard_core::ProblemCode::Cancelled,
				"Cancelled before creation.",
			));
		}
		Ok(())
	}
}

#[test]
fn cancellation_before_first_effect_preserves_all_task_bytes() {
	let fixture = tempfile::tempdir().unwrap();
	let api = repo(fixture.path(), "api");
	let web = repo(fixture.path(), "web");
	let root = fixture.path().join("workspace");
	create(&root, &api);
	let before = state_bytes(&root);
	let store = workyard_state::Store::open_existing(&root.join(".workyard"), None).unwrap();
	let result = smol::block_on(workyard_core::amend(
		request(vec![repository(&web, None)]),
		&workyard_git::Runner::default(),
		&workyard_core::Control::new(|| false),
		&CancelCreation,
		&store,
	))
	.unwrap();
	assert_eq!(
		result.operation.outcome,
		workyard_core::OperationOutcome::Cancelled
	);
	assert_eq!(before, state_bytes(&root));
	assert!(!root.join("tasks/feat/task/web").exists());
}

#[test]
fn legacy_amendment_recovery_requires_the_original_branch_before_settlement() {
	let fixture = tempfile::tempdir().unwrap();
	let api = repo(fixture.path(), "api");
	let web = repo(fixture.path(), "web");
	let root = fixture.path().join("workspace");
	create(&root, &api);
	assert_ok(&workyard(
		&root,
		&["amend", "feat/task", "--from", "main", "--repo", text(&web)],
	));

	let state = root.join(".workyard");
	let filename = workyard_state::task_filename("feat/task");
	let revision_path = state
		.join("evidence")
		.join(&filename)
		.with_extension("revision");
	let mut revision: serde_json::Value =
		serde_json::from_slice(&std::fs::read(&revision_path).unwrap()).unwrap();
	assert!(revision["current"]["pending"].is_null());
	revision["request"]["pull"] = true.into();
	let task = &mut revision["current"]["task"];
	task["operation"]["outcome"] = "failed".into();
	task["participants"][1]["creation"]["pull"] = true.into();
	task["participants"][1]["phase"] = "pulling".into();
	std::fs::write(
		state.join("tasks").join(filename),
		serde_json::to_vec(task).unwrap(),
	)
	.unwrap();
	std::fs::write(revision_path, serde_json::to_vec(&revision).unwrap()).unwrap();

	let tree = root.join("tasks/feat/task/web");
	git(&tree, &["checkout", "-b", "other"]);
	let before = state_bytes(&root);
	let refused = workyard(&root, &["recover", "feat/task"]);
	assert!(!refused.status.success());
	assert_eq!(git(&tree, &["branch", "--show-current"]), "other");
	assert_eq!(state_bytes(&root), before);

	git(&tree, &["checkout", "feat/task"]);
	assert_ok(&workyard(&root, &["recover", "feat/task"]));
	let saved = crate::support::record(&state);
	assert_eq!(saved["participants"][1]["phase"], "completed");
	assert_eq!(saved["operation"]["outcome"], "succeeded");
}

#[test]
fn recovery_checks_earlier_membership_after_later_amendments() {
	let fixture = tempfile::tempdir().unwrap();
	let api = repo(fixture.path(), "api");
	let web = repo(fixture.path(), "web");
	let worker = repo(fixture.path(), "worker");
	let root = fixture.path().join("workspace");
	create(&root, &api);
	for source in [&web, &worker] {
		assert_ok(&workyard(
			&root,
			&[
				"amend",
				"feat/task",
				"--from",
				"main",
				"--repo",
				text(source),
			],
		));
	}

	git(
		&web,
		&[
			"worktree",
			"remove",
			text(&root.join("tasks/feat/task/web")),
		],
	);
	let before = state_bytes(&root);
	let recovered = workyard(&root, &["recover", "feat/task"]);
	assert!(!recovered.status.success());
	assert!(!String::from_utf8_lossy(&recovered.stdout).contains("Nothing to recover"));
	assert_eq!(before, state_bytes(&root));
	assert!(!root.join("tasks/feat/task/web").exists());
}

#[test]
fn dependent_existing_renames_preserve_identity_and_files_in_reverse_input_order() {
	let fixture = tempfile::tempdir().unwrap();
	let api = repo(fixture.path(), "api");
	let web = repo(fixture.path(), "web");
	let root = fixture.path().join("workspace");
	create(&root, &api);
	assert_ok(&workyard(
		&root,
		&["amend", "feat/task", "--from", "main", "--repo", text(&web)],
	));
	std::fs::write(root.join("tasks/feat/task/api/authored"), "api work").unwrap();
	std::fs::write(root.join("tasks/feat/task/web/authored"), "web work").unwrap();
	let before = crate::support::record(&root.join(".workyard"));

	assert_ok(&workyard(
		&root,
		&[
			"amend",
			"feat/task",
			"--repo",
			text(&api),
			"--repo-name",
			"web",
			"--repo",
			text(&web),
			"--repo-name",
			"legacy-web",
		],
	));
	let after = crate::support::record(&root.join(".workyard"));
	for index in 0..2 {
		assert_eq!(
			before["participants"][index]["backing_identity"],
			after["participants"][index]["backing_identity"]
		);
		assert_eq!(
			before["participants"][index]["worktree_identity"],
			after["participants"][index]["worktree_identity"]
		);
	}
	assert_eq!(
		std::fs::read_to_string(root.join("tasks/feat/task/web/authored")).unwrap(),
		"api work"
	);
	assert_eq!(
		std::fs::read_to_string(root.join("tasks/feat/task/legacy-web/authored")).unwrap(),
		"web work"
	);
}
