use rpc::invoke;
#[path = "support/rpc.rs"]
mod rpc;
mod support;
use std::{fs, path::Path};
use support::*;

#[test]
fn create_and_amend_rpc_report_all_optional_failures_without_persisting_them() {
	for action in ["create", "amend"] {
		let fixture = tempfile::tempdir().unwrap();
		let api = repo(fixture.path(), "api");
		let web = repo(fixture.path(), "web");
		let root = fixture.path().join("workspace");
		init_workspace(&root);
		if action == "amend" {
			let original = repo(fixture.path(), "original");
			assert_ok(&workyard(
				&root,
				&[
					"create",
					"feat/task",
					"--from",
					"main",
					"--repo",
					text(&original),
				],
			));
		}

		let response = invoke(
			&root,
			action,
			serde_json::json!({
				"task":"feat/task", "pull":true, "from":"main",
				"repositories":[{"path":api}, {"path":web}]
			}),
		);
		let result = &response["error"]["data"];
		assert_eq!(result["operation"]["outcome"], "failed", "{response}");
		assert_eq!(result["pull_failures"].as_array().unwrap().len(), 2);
		assert_eq!(result["pull_failures"][0]["participant"], "api");
		assert_eq!(result["pull_failures"][1]["participant"], "web");
		assert!(
			result["participants"]
				.as_array()
				.unwrap()
				.iter()
				.all(|p| p["phase"] == "completed")
		);
		let saved = record(&root.join(".workyard"));
		assert!(saved["problem"].is_null());
		assert!(saved.get("pull_failures").is_none());

		let repeat = workyard(
			&root,
			&[
				"amend",
				"feat/task",
				"--pull",
				"--repo",
				text(&api),
				"--repo",
				text(&web),
			],
		);
		assert_ok(&repeat);
		assert!(String::from_utf8_lossy(&repeat.stdout).contains("No changes needed"));
		assert_ok(&workyard(&root, &["recover", "feat/task"]));
	}
}

#[test]
fn pull_conflict_is_current_git_damage_not_incomplete_membership() {
	let fixture = tempfile::tempdir().unwrap();
	let (api, remote) = remote_clone(fixture.path(), "api");
	git(&api, &["config", "user.email", "fixture@example.invalid"]);
	git(&api, &["config", "user.name", "Fixture"]);
	git(&api, &["config", "pull.rebase", "false"]);
	git(&api, &["checkout", "-b", "feat/task"]);
	fs::write(api.join("file"), "remote change\n").unwrap();
	git(&api, &["commit", "-am", "remote change"]);
	git(&api, &["push", text(&remote), "feat/task:main"]);
	git(&api, &["checkout", "main"]);
	git(&api, &["branch", "-D", "feat/task"]);
	fs::write(api.join("file"), "local change\n").unwrap();
	git(&api, &["commit", "-am", "local change"]);
	// A newly created task branch tracks remote main by explicit Git config.
	git(&api, &["config", "branch.feat/task.remote", "origin"]);
	git(
		&api,
		&["config", "branch.feat/task.merge", "refs/heads/main"],
	);
	let root = fixture.path().join("workspace");
	init_workspace(&root);

	let output = workyard(
		&root,
		&[
			"create",
			"feat/task",
			"--repo",
			text(&api),
			"--from",
			"main",
			"--pull",
		],
	);
	assert!(!output.status.success());
	let saved = record(&root.join(".workyard"));
	assert_eq!(saved["participants"][0]["phase"], "completed");
	assert!(saved["problem"].is_null());
	let status = workyard(&root, &["status"]);
	let rendered = String::from_utf8_lossy(&status.stdout);
	assert!(rendered.contains("conflicted"), "{rendered}");
	assert!(!rendered.contains("healthy"));

	git(&root.join("tasks/feat/task/api"), &["merge", "--abort"]);
	let status = workyard(&root, &["status"]);
	assert_ok(&status);
	assert!(String::from_utf8_lossy(&status.stdout).contains("healthy"));
}

/// Interrupt only after Git addition has settled, before optional pull starts.
struct StopBeforePull;
impl workyard_core::RemovalObserver for StopBeforePull {}

impl workyard_core::Observer for StopBeforePull {
	async fn phase(&self, _: &str, _: &str, phase: &str) -> Result<(), workyard_core::Problem> {
		if matches!(phase, "worktree_created" | "added") {
			return Err(workyard_core::Problem::new(
				workyard_core::ProblemCode::Cancelled,
				"Stop before optional pull",
			));
		}
		Ok(())
	}
}

fn interrupted_addition(root: &Path, source: &Path, action: &str) {
	let repositories = vec![workyard_core::RepositoryInput {
		registration_roots: Vec::new(),
		expected_identity: None,
		path: source.into(),
		name: None,
		remote: None,
		from: Some("main".into()),
	}];
	let runner = workyard_git::Runner::default();
	let control = workyard_core::Control::default();
	if action == "create" {
		let store = workyard_state::Store::open_existing(&root.join(".workyard"), None).unwrap();
		let request = workyard_core::CreateRequest {
			task: "feat/task".into(),
			repositories,
			tasks_root: root.join("tasks"),
			state_dir: root.join(".workyard"),
			checkout: false,
			no_fetch: false,
			pull: true,
			remote: None,
			from: None,
		};
		let result = smol::block_on(workyard_core::create(
			request,
			&runner,
			&control,
			&StopBeforePull,
			&store,
		))
		.unwrap();
		assert_eq!(
			result.operation.outcome,
			workyard_core::OperationOutcome::Cancelled
		);
	} else {
		let store = workyard_state::Store::open_existing(&root.join(".workyard"), None).unwrap();
		let request = workyard_core::AmendRequest {
			task: "feat/task".into(),
			repositories,
			remove: Vec::new(),
			removal_options: Default::default(),
			options: workyard_core::CreationOptions {
				checkout: false,
				no_fetch: false,
				pull: true,
			},
			remote: None,
			from: None,
		};
		let result = smol::block_on(workyard_core::amend(
			request,
			&runner,
			&control,
			&StopBeforePull,
			&store,
		))
		.unwrap();
		assert_eq!(
			result.operation.outcome,
			workyard_core::OperationOutcome::Cancelled
		);
	}
}

#[test]
fn both_recovery_routes_report_optional_pull_failures_through_cli_and_rpc() {
	for action in ["create", "amend"] {
		for interface in ["cli", "rpc"] {
			let fixture = tempfile::tempdir().unwrap();
			let api = repo(fixture.path(), "api");
			let root = fixture.path().join("workspace");
			init_workspace(&root);
			if action == "amend" {
				let original = repo(fixture.path(), "original");
				assert_ok(&workyard(
					&root,
					&[
						"create",
						"feat/task",
						"--from",
						"main",
						"--repo",
						text(&original),
					],
				));
			}
			interrupted_addition(&root, &api, action);
			let before = record(&root.join(".workyard"));
			let participant = before["participants"]
				.as_array()
				.unwrap()
				.iter()
				.find(|p| p["name"] == "api")
				.unwrap();
			assert_eq!(participant["phase"], "worktree_created");
			assert!(root.join("tasks/feat/task/api/.git").is_file());

			if interface == "cli" {
				let output = workyard(&root, &["recover", "feat/task"]);
				assert!(!output.status.success());
				let rendered = String::from_utf8_lossy(&output.stdout);
				assert!(
					rendered.contains("Recovery completed with pull failures"),
					"{rendered}"
				);
				assert!(rendered.contains("Pull failed for api"), "{rendered}");
				assert!(rendered.contains("git_failed"), "{rendered}");
			} else {
				let response = invoke(&root, "recover", serde_json::json!({"task":"feat/task"}));
				let result = &response["error"]["data"];
				assert_eq!(result["operation"]["kind"], "recover", "{response}");
				assert_eq!(result["operation"]["outcome"], "failed");
				assert_eq!(result["pull_failures"][0]["participant"], "api");
				assert_eq!(result["pull_failures"][0]["problem"]["code"], "git_failed");
				assert!(
					result["pull_failures"][0]["problem"]["message"]
						.as_str()
						.unwrap()
						.contains("git")
				);
				assert!(
					result["participants"]
						.as_array()
						.unwrap()
						.iter()
						.all(|p| p["phase"] == "completed")
				);
			}

			let saved = record(&root.join(".workyard"));
			assert!(
				saved["participants"]
					.as_array()
					.unwrap()
					.iter()
					.all(|p| p["phase"] == "completed")
			);
			assert_eq!(saved["operation"]["outcome"], "succeeded");
			assert!(saved["problem"].is_null());
			assert!(saved.get("pull_failures").is_none());
			let repeated = workyard(&root, &["recover", "feat/task"]);
			assert_ok(&repeated);
			assert!(String::from_utf8_lossy(&repeated.stdout).contains("Nothing to recover"));
		}
	}
}
