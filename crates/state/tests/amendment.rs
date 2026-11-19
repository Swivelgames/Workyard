mod support;
use std::fs;
use workyard_core::{
	AmendRequest, AmendState, CreationOptions, EffectState, OperationOutcome, Participant, Phase,
	RecoveryState, RepositoryInput, State,
};

fn established(workspace: &support::Workspace) -> workyard_core::Task {
	let mut task = support::task(&workspace.root, &workspace.state, "feat/task");
	task.generation = 1;
	task.operation.outcome = OperationOutcome::Succeeded;
	task.participants.push(Participant {
		placement: None,
		creation: None,
		name: "api".into(),
		source: workspace.root.join("source"),
		common_dir: workspace.root.join("source/.git"),
		backing_identity: "backing".into(),
		registration_roots: Vec::new(),
		worktree_identity: Some("worktree".into()),
		destination: workspace.root.join("feat/task/api"),
		remote: None,
		from: Some("main".into()),
		phase: Phase::Completed,
	});
	task
}

fn request(task: &workyard_core::Task) -> AmendRequest {
	AmendRequest {
		task: task.name.clone(),
		repositories: vec![RepositoryInput {
			registration_roots: Vec::new(),
			expected_identity: None,
			path: task.participants[0].source.clone(),
			name: Some("legacy".into()),
			remote: None,
			from: None,
		}],
		options: CreationOptions {
			checkout: false,
			no_fetch: true,
			pull: false,
		},
		remote: None,
		from: None,
	}
}

#[test]
fn revision_effect_survives_publication_gap_and_reader_does_not_repair_it() {
	let workspace = support::Workspace::new();
	let store = &workspace.store;
	let guard = store.admit("feat/task").unwrap();
	let mut task = established(&workspace);
	store.begin(&guard, &task).unwrap();
	let original_admission = fs::read(&guard.evidence_path).unwrap();
	let original_record = fs::read(&guard.record_path).unwrap();
	store.begin_amend(&guard, &task, &request(&task)).unwrap();
	task.participants[0].name = "legacy".into();
	task.participants[0].destination = workspace.root.join("feat/task/legacy");
	store
		.record_effect(&guard, &task, 0, EffectState::Settled)
		.unwrap();
	let revision_path = guard.evidence_path.with_extension("revision");
	let revision = fs::read(&revision_path).unwrap();
	let loaded = store.load(&guard, "feat/task").unwrap();
	assert_eq!(loaded.task.participants[0].name, "legacy");
	assert!(loaded.needs_publication);
	drop(guard);
	let reader = workyard_state::Reader::open(&workspace.state, None).unwrap();
	let snapshots = reader.tasks(Some("feat/task")).unwrap();
	assert!(snapshots[0].problems.is_empty());
	assert_eq!(
		snapshots[0].task.as_ref().unwrap().participants[0].name,
		"legacy"
	);
	assert_eq!(fs::read(&revision_path).unwrap(), revision);
	let guard = store.admit("feat/task").unwrap();
	assert_eq!(fs::read(&guard.evidence_path).unwrap(), original_admission);
	assert_eq!(fs::read(&guard.record_path).unwrap(), original_record);
	store.save(&guard, &loaded.task).unwrap();
	assert!(!store.load(&guard, "feat/task").unwrap().needs_publication);
}

#[test]
fn failed_task_publication_retains_revision_and_pending_writers() {
	let workspace = support::Workspace::new();
	let store = &workspace.store;
	let guard = store.admit("feat/task").unwrap();
	let task = established(&workspace);
	store.begin(&guard, &task).unwrap();
	store.begin_amend(&guard, &task, &request(&task)).unwrap();
	store
		.record_effect(&guard, &task, 0, EffectState::Pending)
		.unwrap();
	fs::remove_file(&guard.record_path).unwrap();
	fs::create_dir(&guard.record_path).unwrap();
	assert!(store.save(&guard, &task).is_err());
	fs::remove_dir(&guard.record_path).unwrap();
	let loaded = store.load(&guard, "feat/task").unwrap();
	assert_eq!(loaded.pending, Some(0));
	assert_eq!(loaded.task.participants[0].name, "api");
	assert!(loaded.recorded.is_none());
}

#[test]
fn unchanged_amend_restores_exact_prior_revision_bytes() {
	let workspace = support::Workspace::new();
	let store = &workspace.store;
	let guard = store.admit("feat/task").unwrap();
	let task = established(&workspace);
	store.begin(&guard, &task).unwrap();
	store.begin_amend(&guard, &task, &request(&task)).unwrap();
	let path = guard.evidence_path.with_extension("revision");
	let value: serde_json::Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
	let compact = serde_json::to_vec(&value).unwrap();
	fs::write(&path, &compact).unwrap();
	let checkpoint = store.begin_amend(&guard, &task, &request(&task)).unwrap();
	store.unchanged_amend(&guard, checkpoint).unwrap();
	assert_eq!(fs::read(path).unwrap(), compact);
}
