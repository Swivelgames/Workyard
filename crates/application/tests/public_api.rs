use trine::{Cancellation, InvocationContext, NativePath};
use workyard_application::{CreateInput, CreateOutcome, Progress, RepositoryInput, create};

fn invalid_request() -> CreateInput {
	CreateInput {
		task: "../invalid".into(),
		repositories: vec![RepositoryInput {
			path: NativePath::new(std::path::PathBuf::from("unused-repository")).unwrap(),
			name: None,
			remote: None,
			from: None,
		}],
		tasks_root: None,
		state_dir: None,
		checkout: false,
		no_fetch: true,
		pull: false,
		remote: None,
		from: None,
	}
}

#[test]
fn root_and_capability_imports_support_direct_invocation() {
	let context = || InvocationContext::<Progress>::unobserved(Cancellation::new());

	// Both public paths reach validation before filesystem or Git effects.
	let root: Result<CreateOutcome, CreateOutcome> =
		smol::block_on(create(invalid_request(), context()));
	let scoped: Result<create::CreateOutcome, create::CreateOutcome> =
		smol::block_on(create::create(invalid_request(), context()));

	for result in [root, scoped] {
		let outcome = result.err().expect("invalid task must be rejected");
		assert!(outcome.record.is_none());
		assert!(outcome.participants.is_empty());
		assert!(outcome.problem.is_some());
	}
}

#[test]
fn action_outcomes_keep_distinct_types_and_explicit_lifecycle_operation_fields() {
	let task = workyard_core::Task {
		name: "feat/test".into(),
		root: "workspace".into(),
		state_dir: "state".into(),
		participants: vec![workyard_core::Participant {
			name: "api".into(),
			source: "source".into(),
			common_dir: "source/.git".into(),
			backing_identity: "identity".into(),
			worktree_identity: Some("tree".into()),
			destination: "workspace/api".into(),
			remote: None,
			from: None,
			phase: workyard_core::Phase::Completed,
		}],
		checkout: false,
		no_fetch: true,
		pull: false,
		lifecycle: workyard_core::TaskLifecycle::Active,
		operation: workyard_core::Operation {
			outcome: workyard_core::OperationOutcome::Succeeded,
			..workyard_core::Operation::create()
		},
		problem: None,
	};
	let create = CreateOutcome::from_task(task.clone(), "state/task.json".into());
	let recover =
		workyard_application::recover::RecoverOutcome::from_task(task, "state/task.json".into());
	let expected = serde_json::json!({
		"operation": {"kind":"create","continues":null,"outcome":"succeeded"}, "lifecycle":"active", "task": "feat/test", "tasks_root":"workspace", "state_dir":"state", "record": "state/task.json",
		"participants": [{"name":"api", "path":"workspace/api", "phase":"completed"}]
	});
	assert_eq!(serde_json::to_value(create).unwrap(), expected);
	assert_eq!(serde_json::to_value(recover).unwrap(), expected);
}
