mod support;
use std::fs;
use workyard_core::{OperationOutcome, Participant, Phase, ProblemCode, RecoveryState, Task};
use workyard_state::{Store, discover_state};

fn completed_task(workspace: &support::Workspace) -> Task {
	let mut task = support::task(&workspace.root, &workspace.state, "feat/task");
	task.operation.outcome = OperationOutcome::Succeeded;
	task.participants.push(Participant {
		creation: None,
		name: "api".into(),
		source: workspace.root.join("source"),
		common_dir: workspace.root.join("source/.git"),
		backing_identity: "backing".into(),
		worktree_identity: Some("worktree".into()),
		destination: workspace.root.join("feat/task/api"),
		remote: None,
		from: Some("main".into()),
		phase: Phase::Completed,
	});
	task
}

#[test]
fn recovery_requires_admission_and_preserves_corrupt_effect_evidence() {
	let workspace = support::Workspace::new();
	let store = &workspace.store;
	let guard = store.admit("feat/task").unwrap();
	assert_eq!(
		store.load(&guard, "feat/task").unwrap_err().code,
		ProblemCode::TaskUnknown
	);
	store.begin(&guard, &completed_task(&workspace)).unwrap();
	let effect = guard.evidence_path.with_extension("effects");
	let original = fs::read(&effect).unwrap();
	fs::write(&effect, "corrupt").unwrap();
	assert_eq!(
		store.load(&guard, "feat/task").unwrap_err().code,
		ProblemCode::DamagedState
	);
	assert_eq!(fs::read(&effect).unwrap(), b"corrupt");
	let mut newer: serde_json::Value = serde_json::from_slice(&original).unwrap();
	newer["version"] = 99.into();
	fs::write(&effect, serde_json::to_vec(&newer).unwrap()).unwrap();
	assert_eq!(
		store.load(&guard, "feat/task").unwrap_err().code,
		ProblemCode::IncompatibleState
	);
	fs::remove_file(&effect).unwrap();
	fs::remove_file(&guard.record_path).unwrap();
	assert_eq!(
		store.load(&guard, "feat/task").unwrap_err().code,
		ProblemCode::DamagedState
	);
	assert!(guard.evidence_path.exists());
}

#[test]
fn legacy_completed_records_load_but_uncertain_effects_remain_diagnostic() {
	let workspace = support::Workspace::new();
	let store = &workspace.store;
	let guard = store.admit("feat/task").unwrap();
	let mut original = completed_task(&workspace);
	original.participants[0].worktree_identity = None;
	store.begin(&guard, &original).unwrap();
	fs::remove_file(guard.evidence_path.with_extension("effects")).unwrap();
	assert_eq!(
		store.load(&guard, "feat/task").unwrap().task.participants[0].phase,
		Phase::Completed
	);
	for phase in [Phase::Creating, Phase::Pulling] {
		original.participants[0].phase = phase;
		store.save(&guard, &original).unwrap();
		assert_eq!(
			store.load(&guard, "feat/task").unwrap_err().code,
			ProblemCode::DamagedState
		);
	}
	original.participants[0].phase = Phase::Fetching;
	store.save(&guard, &original).unwrap();
	assert!(store.load(&guard, "feat/task").is_ok());
}

#[test]
fn explicit_workspace_wins_and_missing_selection_does_not_bootstrap() {
	let workspace = support::Workspace::new();
	let nested = workspace.root.join("nested");
	fs::create_dir(&nested).unwrap();
	assert_eq!(discover_state(&nested, None).unwrap(), workspace.state);

	let other = workspace.temporary.path().join("other");
	let other_root = workyard_state::initialize_workspace(&other, None).unwrap();
	let state = discover_state(&nested, Some(&other)).unwrap();
	assert_eq!(state, other.join(".workyard"));
	assert!(Store::open_existing(&state, Some(&other_root)).is_ok());
	assert_eq!(
		Store::open_existing(&state, Some(&workspace.root))
			.err()
			.unwrap()
			.code,
		ProblemCode::WorkspaceBindingConflict
	);

	let missing = workspace.temporary.path().join("missing");
	assert!(discover_state(&nested, Some(&missing)).is_err());
	assert!(!missing.exists());
}

#[test]
fn loading_preserves_status_and_problem_before_recovery_prepares_attempt() {
	let workspace = support::Workspace::new();
	let store = &workspace.store;
	let guard = store.admit("feat/task").unwrap();
	let mut original = completed_task(&workspace);
	original.problem = Some(
		workyard_core::Problem::new(ProblemCode::GitFailed, "pull failed")
			.remedy("inspect worktree"),
	);
	store.begin(&guard, &original).unwrap();
	fs::remove_file(guard.evidence_path.with_extension("effects")).unwrap();

	for status in [OperationOutcome::Failed, OperationOutcome::Cancelled] {
		original.operation.outcome = status;
		store.save(&guard, &original).unwrap();

		let loaded = store.load(&guard, "feat/task").unwrap().task;
		assert_eq!(loaded.operation.outcome, status);
		let problem = loaded.problem.unwrap();
		assert_eq!(problem.code, ProblemCode::GitFailed);
		assert_eq!(problem.message, "pull failed");
		assert_eq!(problem.remedy, "inspect worktree");
	}

	workyard_core::State::record_effect(
		store,
		&guard,
		&original,
		0,
		workyard_core::EffectState::Settled,
	)
	.unwrap();

	let effect = guard.evidence_path.with_extension("effects");
	let mut value: serde_json::Value = serde_json::from_slice(&fs::read(&effect).unwrap()).unwrap();
	value["task"]["problem"]["code"] = "unknown_code".into();
	let bytes = serde_json::to_vec(&value).unwrap();
	fs::write(&effect, &bytes).unwrap();
	assert_eq!(
		store.load(&guard, "feat/task").unwrap_err().code,
		ProblemCode::DamagedState
	);
	assert_eq!(fs::read(effect).unwrap(), bytes);
}

#[test]
fn legacy_task_normalization_serializes_canonical_v3_and_rejects_mixed_fields() {
	let workspace = support::Workspace::new();
	let task = completed_task(&workspace);
	let mut value = serde_json::to_value(workyard_state::TaskRecord::from(&task)).unwrap();
	value.as_object_mut().unwrap().remove("lifecycle");
	value.as_object_mut().unwrap().remove("operation");
	value.as_object_mut().unwrap().remove("generation");
	value.as_object_mut().unwrap().remove("removal");
	value["version"] = 1.into();
	value["status"] = "completed".into();
	let normalized: workyard_state::TaskRecord = serde_json::from_value(value.clone()).unwrap();
	let canonical = serde_json::to_value(normalized).unwrap();
	assert_eq!(canonical["version"], 3);
	assert_eq!(canonical["lifecycle"], "active");
	assert_eq!(canonical["operation"]["outcome"], "succeeded");
	assert!(canonical["operation"]["kind"].is_null());
	assert!(canonical.get("status").is_none());
	assert!(serde_json::from_value::<workyard_state::TaskRecord>(canonical.clone()).is_ok());

	value["operation"] = serde_json::Value::Null;
	assert!(serde_json::from_value::<workyard_state::TaskRecord>(value).is_err());
	let mut mixed = canonical;
	mixed["status"] = serde_json::Value::Null;
	assert!(serde_json::from_value::<workyard_state::TaskRecord>(mixed).is_err());
}

#[test]
fn invalid_v2_lifecycle_or_operation_cannot_authorize_recovery() {
	let workspace = support::Workspace::new();
	let store = &workspace.store;
	let guard = store.admit("feat/task").unwrap();
	store.begin(&guard, &completed_task(&workspace)).unwrap();
	let original = fs::read(&guard.record_path).unwrap();
	for (field, bad) in [
		("lifecycle", serde_json::json!("completed")),
		(
			"operation",
			serde_json::json!({"kind":"create","continues":"create","outcome":"succeeded"}),
		),
		(
			"operation",
			serde_json::json!({"kind":"unknown_future","continues":null,"outcome":"succeeded"}),
		),
		(
			"operation",
			serde_json::json!({"kind":"create","continues":null,"outcome":"unknown_future"}),
		),
	] {
		let mut value: serde_json::Value = serde_json::from_slice(&original).unwrap();
		value[field] = bad;
		let bytes = serde_json::to_vec(&value).unwrap();
		fs::write(&guard.record_path, &bytes).unwrap();
		assert_eq!(
			store.load(&guard, "feat/task").unwrap_err().code,
			ProblemCode::DamagedState
		);
		assert_eq!(fs::read(&guard.record_path).unwrap(), bytes);
	}
}

#[test]
fn unsupported_task_and_nested_effect_versions_are_not_reset() {
	let workspace = support::Workspace::new();
	let store = &workspace.store;
	let guard = store.admit("feat/task").unwrap();
	store.begin(&guard, &completed_task(&workspace)).unwrap();
	let original = fs::read(&guard.record_path).unwrap();
	let mut task: serde_json::Value = serde_json::from_slice(&original).unwrap();
	task["version"] = 99.into();
	let bytes = serde_json::to_vec(&task).unwrap();
	fs::write(&guard.record_path, &bytes).unwrap();
	assert_eq!(
		store.load(&guard, "feat/task").unwrap_err().code,
		ProblemCode::IncompatibleState
	);
	assert_eq!(fs::read(&guard.record_path).unwrap(), bytes);

	fs::write(&guard.record_path, original).unwrap();
	let effects = guard.evidence_path.with_extension("effects");
	let mut effect: serde_json::Value =
		serde_json::from_slice(&fs::read(&effects).unwrap()).unwrap();
	effect["task"]["version"] = 99.into();
	let bytes = serde_json::to_vec(&effect).unwrap();
	fs::write(&effects, &bytes).unwrap();
	assert!(store.load(&guard, "feat/task").is_err());
	assert_eq!(fs::read(&effects).unwrap(), bytes);
}
