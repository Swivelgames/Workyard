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
		remove: Vec::new(),
		removal_options: Default::default(),
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
	store
		.begin_amend(&guard, &task, &request(&task), &[])
		.unwrap();
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
	store
		.begin_amend(&guard, &task, &request(&task), &[])
		.unwrap();
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
	store
		.begin_amend(&guard, &task, &request(&task), &[])
		.unwrap();
	let path = guard.evidence_path.with_extension("revision");
	let value: serde_json::Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
	let compact = serde_json::to_vec(&value).unwrap();
	fs::write(&path, &compact).unwrap();
	let checkpoint = store
		.begin_amend(&guard, &task, &request(&task), &[])
		.unwrap();
	store.unchanged_amend(&guard, checkpoint).unwrap();
	assert_eq!(fs::read(path).unwrap(), compact);
}

#[test]
fn confirmed_unlink_survives_stale_publication_and_a_failed_readdition() {
	let workspace = support::Workspace::new();
	let store = &workspace.store;
	let guard = store.admit("feat/task").unwrap();
	let mut task = established(&workspace);
	let original = task.participants[0].clone();
	let mut sibling = original.clone();
	sibling.name = "web".into();
	sibling.backing_identity = "web-backing".into();
	sibling.worktree_identity = Some("web-admin".into());
	sibling.destination = workspace.root.join("feat/task/web");
	task.participants.push(sibling);
	store.begin(&guard, &task).unwrap();

	let target = workyard_core::RemovalTarget {
		name: original.name.clone(),
		backing_identity: original.backing_identity.clone(),
		worktree_identity: original.worktree_identity.clone().unwrap(),
	};
	let mut removal = request(&task);
	removal.repositories.clear();
	removal.remove = vec![target.name.clone()];
	removal.removal_options.force = true;
	store
		.begin_amend(&guard, &task, &removal, std::slice::from_ref(&target))
		.unwrap();
	store
		.record_effect(&guard, &task, 0, EffectState::Pending)
		.unwrap();

	task.participants.remove(0);
	store.settle_unlink(&guard, &task, &target).unwrap();
	let interrupted = store.load(&guard, &task.name).unwrap();
	assert!(interrupted.needs_publication);
	assert!(interrupted.pending.is_none());
	assert_eq!(interrupted.task.participants.len(), 1);
	assert_eq!(interrupted.task.participants[0].name, "web");
	assert_eq!(interrupted.recorded.unwrap().participants.len(), 2);
	assert_eq!(store.removal_targets(&guard).unwrap(), [target]);
	assert!(
		!store
			.amend_request(&guard)
			.unwrap()
			.unwrap()
			.removal_options
			.force
	);

	let add = request(&task);
	store.begin_amend(&guard, &task, &add, &[]).unwrap();
	let mut attempting = original;
	attempting.phase = Phase::Creating;
	attempting.worktree_identity = None;
	task.participants.push(attempting);
	store
		.record_effect(&guard, &task, 1, EffectState::Pending)
		.unwrap();
	task.participants.pop();
	store
		.record_effect(&guard, &task, 0, EffectState::Settled)
		.unwrap();
	assert_eq!(
		store.load(&guard, &task.name).unwrap().task.participants,
		interrupted.task.participants
	);

	// Deleting the independent unlink receipt must expose the missing admission.
	let path = guard.evidence_path.with_extension("revision");
	let mut revision: serde_json::Value =
		serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
	revision.as_object_mut().unwrap().remove("removed");
	fs::write(path, serde_json::to_vec(&revision).unwrap()).unwrap();
	assert_eq!(
		store.load(&guard, &task.name).unwrap_err().code,
		workyard_core::ProblemCode::DamagedState
	);
}

#[test]
fn stored_removal_cannot_bind_one_participants_name_to_anothers_identity() {
	let workspace = support::Workspace::new();
	let mut task = established(&workspace);
	let mut web = task.participants[0].clone();
	web.name = "web".into();
	web.backing_identity = "web-backing".into();
	web.worktree_identity = Some("web-admin".into());
	web.destination = workspace.root.join("feat/task/web");
	task.participants.push(web);
	let store = &workspace.store;
	let guard = store.admit(&task.name).unwrap();
	store.begin(&guard, &task).unwrap();

	let mut request = request(&task);
	request.repositories.clear();
	request.remove.push("web".into());
	let target = workyard_core::RemovalTarget {
		name: "web".into(),
		backing_identity: "web-backing".into(),
		worktree_identity: "web-admin".into(),
	};
	store
		.begin_amend(&guard, &task, &request, &[target])
		.unwrap();

	let path = guard.evidence_path.with_extension("revision");
	let mut revision: serde_json::Value =
		serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
	revision["request"]["remove"][0]["name"] = "api".into();
	fs::write(path, serde_json::to_vec(&revision).unwrap()).unwrap();
	assert_eq!(
		store.load(&guard, &task.name).unwrap_err().code,
		workyard_core::ProblemCode::DamagedState
	);
}
