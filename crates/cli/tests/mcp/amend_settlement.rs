//! Cancel progress at the exact point an amendment has verified a new worktree.
use super::{
	client::Client,
	coordination::{StopAtPhase, task_documents},
	support,
};
use serde_json::json;
use workyard_core::{
	AmendRequest, Control, CreationOptions, OperationOutcome, RemovalOptions, RepositoryInput,
};

const TASK: &str = "feat/additions";

fn cancel_after_first_addition(additions: &[&str]) -> tempfile::TempDir {
	let fixture = tempfile::tempdir().unwrap();
	let root = fixture.path().join("workspace");
	support::init_workspace(&root);
	let api = support::repo(fixture.path(), "api");
	let created = support::workyard(
		&root,
		&[
			"create",
			TASK,
			"--from",
			"main",
			"--repo",
			support::text(&api),
		],
	);
	support::assert_ok(&created);

	let repositories = additions
		.iter()
		.map(|name| RepositoryInput {
			path: support::repo(fixture.path(), name),
			name: None,
			expected_identity: None,
			registration_roots: Vec::new(),
			remote: None,
			from: None,
		})
		.collect();
	let request = AmendRequest {
		task: TASK.into(),
		repositories,
		remove: Vec::new(),
		removal_options: RemovalOptions::default(),
		options: CreationOptions {
			checkout: false,
			no_fetch: true,
		},
		remote: None,
		from: Some("main".into()),
	};

	let store = workyard_state::Store::open_existing(&root.join(".workyard"), None).unwrap();
	let result = smol::block_on(workyard_core::amend(
		request,
		&workyard_git::Runner::default(),
		&Control::default(),
		&StopAtPhase("added"),
		&store,
	))
	.unwrap();
	assert_eq!(result.operation.outcome, OperationOutcome::Cancelled);
	fixture
}

#[test]
fn cancelling_the_last_added_event_leaves_a_healthy_task_and_nothing_to_recover() {
	let fixture = cancel_after_first_addition(&["web"]);
	let root = fixture.path().join("workspace");
	let settled = task_documents(&root.join(".workyard"));
	let mut client = Client::launch(&root);

	let status = client.call("status", json!({"task": TASK}));
	let task = &status["structuredContent"]["tasks"][0];
	assert_eq!(task["condition"], "healthy", "{status}");
	let web = &task["recorded"]["participants"][1];
	assert_eq!(web["name"], "web");
	assert_eq!(web["phase"], "completed");

	let recovered = client.call("recover", json!({"task": TASK}));
	assert_eq!(
		recovered["structuredContent"]["nothing_to_recover"], true,
		"{recovered}"
	);
	assert_eq!(task_documents(&root.join(".workyard")), settled);
	client.close();
}

#[test]
fn recovery_after_an_earlier_added_event_finishes_the_next_participant() {
	let fixture = cancel_after_first_addition(&["web", "infra"]);
	let root = fixture.path().join("workspace");
	let trees = root.join("tasks").join(TASK);
	assert!(trees.join("web/.git").is_file());
	assert!(!trees.join("infra").exists());

	let mut client = Client::launch(&root);
	let recovered = client.call("recover", json!({"task": TASK}));
	assert_ne!(recovered["isError"], true, "{recovered}");
	assert_eq!(recovered["structuredContent"]["nothing_to_recover"], false);
	assert!(trees.join("web/.git").is_file());
	assert!(trees.join("infra/.git").is_file());
	client.close();
}
