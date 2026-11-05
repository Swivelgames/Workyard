mod support;
use std::fs;
use workyard_core::{Participant, Phase, ProblemCode, RecoveryState, Task, TaskStatus};
use workyard_state::{Store, discover_state};

fn completed_task(workspace: &support::Workspace) -> Task {
	let mut task = support::task(&workspace.root, &workspace.state, "feat/task");
	task.status = TaskStatus::Completed;
	task.participants.push(Participant {
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
		ProblemCode::DamagedState
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
fn explicit_state_wins_and_existing_open_checks_root_without_bootstrap() {
	let workspace = support::Workspace::new();
	let root = &workspace.root;
	let state = &workspace.state;
	let temporary = &workspace.temporary;
	let nested = root.join("nested");
	fs::create_dir(&nested).unwrap();
	assert_eq!(discover_state(&nested, None).unwrap(), *state);
	assert_eq!(discover_state(root, None).unwrap(), *state);
	let external_root = temporary.path().join("external-root");
	let external = temporary.path().join("external-state");
	let external_store = Store::open(&external_root, &external).unwrap();
	assert_eq!(discover_state(&nested, Some(&external)).unwrap(), external);
	assert!(Store::open_existing(&external, Some(&external_root)).is_ok());
	assert_eq!(
		Store::open_existing(&external, Some(root))
			.err()
			.unwrap()
			.code,
		ProblemCode::WorkspaceBindingConflict
	);
	let missing = temporary.path().join("missing");
	assert_eq!(discover_state(&nested, Some(&missing)).unwrap(), missing);
	assert!(Store::open_existing(&missing, None).is_err());
	assert!(!missing.exists());
	drop(external_store);
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

	for status in [TaskStatus::Failed, TaskStatus::Cancelled] {
		original.status = status;
		store.save(&guard, &original).unwrap();

		let loaded = store.load(&guard, "feat/task").unwrap().task;
		assert_eq!(loaded.status, status);
		let problem = loaded.problem.unwrap();
		assert_eq!(problem.code, ProblemCode::GitFailed);
		assert_eq!(problem.message, "pull failed");
		assert_eq!(problem.remedy, "inspect worktree");
	}

	store.begin(&guard, &original).unwrap();

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
