use std::{
	path::{Path, PathBuf},
	sync::{
		Mutex,
		atomic::{AtomicBool, Ordering},
	},
};
use workyard_core::*;

struct Fixture {
	cancel_after_add: bool,
	cancelled: std::sync::Arc<AtomicBool>,
	added: Mutex<Vec<String>>,
	saved: Mutex<Vec<Task>>,
	fail_publish: bool,
	fail_identity: bool,
	pull_error: Option<ProblemCode>,
	effects: Mutex<Vec<(Task, bool)>>,
}

impl Fixture {
	fn new(cancel_after_add: bool, fail_publish: bool) -> Self {
		Self {
			cancel_after_add,
			cancelled: Default::default(),
			added: Default::default(),
			saved: Default::default(),
			fail_publish,
			fail_identity: false,
			pull_error: None,
			effects: Default::default(),
		}
	}
}

impl Git for Fixture {
	async fn worktree_identity(&self, _: &Path) -> Result<Option<String>, Problem> {
		if self.fail_identity {
			return Err(Problem::new(
				ProblemCode::IoError,
				"Injected identity-read failure",
			));
		}
		Ok(Some("administrative-identity".into()))
	}

	async fn validate_branch(&self, _: &Path, _: &str, _: &Control) -> Result<(), Problem> {
		Ok(())
	}

	async fn inspect(
		&self,
		path: &Path,
		_: Option<&str>,
		_: &Control,
	) -> Result<Repository, Problem> {
		Ok(Repository {
			source: path.into(),
			common_dir: path.join(".git"),
			remote: None,
			identity: "fixture".into(),
		})
	}

	async fn revalidate(&self, _: &Repository, _: &Control) -> Result<(), Problem> {
		Ok(())
	}

	async fn plan(
		&self,
		_: &Repository,
		_: &str,
		_: bool,
		_: Option<&str>,
		_: SourceResolution,
		_: &Control,
	) -> Result<BranchPlan, Problem> {
		Ok(BranchPlan::Create("main".into()))
	}

	async fn fetch(&self, _: &Repository, _: &Control) -> Result<(), Problem> {
		Ok(())
	}

	async fn add(
		&self,
		repository: &Repository,
		_: &str,
		_: &Path,
		_: &BranchPlan,
		_: &Control,
	) -> Result<(), Problem> {
		self.added
			.lock()
			.unwrap()
			.push(repository.source.to_string_lossy().into());
		if self.cancel_after_add {
			self.cancelled.store(true, Ordering::SeqCst);
		}
		Ok(())
	}

	async fn pull(&self, _: &Repository, _: &Path, _: &Control) -> Result<(), Problem> {
		if let Some(code) = self.pull_error {
			return Err(Problem::new(code, "Injected pull failure"));
		}
		Ok(())
	}
}

impl State for Fixture {
	fn effect(&self, _: &(), task: &Task, _: usize, settled: bool) -> Result<(), Problem> {
		self.effects.lock().unwrap().push((task.clone(), settled));
		Ok(())
	}

	type Admission = ();
	fn next_generation(&self, _admission: &Self::Admission) -> Result<u64, Problem> {
		Ok(1)
	}
	fn verify(&self) -> Result<(), Problem> {
		Ok(())
	}

	fn admit(&self, _: &str) -> Result<(), Problem> {
		Ok(())
	}

	fn ensure_new(&self, _: &(), _: &str) -> Result<(), Problem> {
		Ok(())
	}

	fn record_path(&self, _: &()) -> PathBuf {
		"state/tasks/task.json".into()
	}

	fn validate_destination(&self, _: &Path) -> Result<(), Problem> {
		Ok(())
	}

	fn reserve(&self, _: &Path) -> Result<(), Problem> {
		Ok(())
	}

	fn begin(&self, _: &(), task: &Task) -> Result<(), Problem> {
		self.saved.lock().unwrap().push(task.clone());
		Ok(())
	}

	fn save(&self, _: &(), task: &Task) -> Result<(), Problem> {
		if self.fail_publish && !self.added.lock().unwrap().is_empty() {
			return Err(Problem::new(
				ProblemCode::PublicationUncertain,
				"Injected post-effect publication failure",
			));
		}
		self.saved.lock().unwrap().push(task.clone());
		Ok(())
	}
}

struct Quiet;
impl Observer for Quiet {
	async fn phase(&self, _: &str, _: &str, _: &str) -> Result<(), Problem> {
		Ok(())
	}
}

struct Broken;
impl Observer for Broken {
	async fn phase(&self, _: &str, _: &str, _: &str) -> Result<(), Problem> {
		Err(Problem::new(ProblemCode::Cancelled, "Presentation failed"))
	}
}

fn request() -> CreateRequest {
	CreateRequest {
		task: "feat/test".into(),
		repositories: ["api", "web"]
			.into_iter()
			.map(|name| RepositoryInput {
				registration_roots: Vec::new(),
				expected_identity: None,
				path: PathBuf::from("repos").join(name),
				name: None,
				remote: None,
				from: Some("main".into()),
			})
			.collect(),
		tasks_root: "workspace".into(),
		state_dir: "state".into(),
		checkout: false,
		no_fetch: true,
		pull: false,
		remote: None,
		from: None,
	}
}

#[test]
fn changed_project_identity_stops_before_task_admission_and_git_effects() {
	let fixture = Fixture::new(false, false);
	let mut input = request();
	input.repositories[1].expected_identity = Some("replaced".into());

	let result = smol::block_on(create(
		input,
		&fixture,
		&Control::default(),
		&Quiet,
		&fixture,
	));
	assert_eq!(result.err().unwrap().code, ProblemCode::RepositoryChanged);
	assert!(fixture.saved.lock().unwrap().is_empty());
	assert!(fixture.added.lock().unwrap().is_empty());
}

#[test]
fn cancellation_racing_success_records_known_worktree_and_stops_next_repo() {
	let fixture = Fixture::new(true, false);
	let cancelled = fixture.cancelled.clone();
	let control = Control::new(move || cancelled.load(Ordering::SeqCst));
	let CreateResult { task, .. } =
		smol::block_on(create(request(), &fixture, &control, &Quiet, &fixture)).unwrap();
	assert_eq!(task.operation.outcome, OperationOutcome::Cancelled);
	assert_eq!(task.participants[0].phase, Phase::WorktreeCreated);
	assert_eq!(task.participants[1].phase, Phase::NotStarted);
	assert_eq!(fixture.added.lock().unwrap().len(), 1);
	assert_eq!(
		fixture.saved.lock().unwrap().last().unwrap().participants[0].phase,
		Phase::WorktreeCreated
	);
}

#[test]
fn post_effect_publication_failure_preserves_evidence_and_stops_later_effects() {
	let fixture = Fixture::new(false, true);
	let CreateResult { task, .. } = smol::block_on(create(
		request(),
		&fixture,
		&Control::default(),
		&Quiet,
		&fixture,
	))
	.unwrap();
	assert_eq!(task.operation.outcome, OperationOutcome::Failed);
	assert_eq!(
		task.problem.unwrap().code,
		ProblemCode::PublicationUncertain
	);
	assert_eq!(task.participants[0].phase, Phase::WorktreeCreated);
	assert_eq!(task.participants[1].phase, Phase::NotStarted);
	assert_eq!(fixture.added.lock().unwrap().len(), 1);
	assert_eq!(
		fixture.saved.lock().unwrap().last().unwrap().participants[0].phase,
		Phase::Creating
	);
}

#[test]
fn broken_presentation_stops_before_effect_and_records_cancellation() {
	let fixture = Fixture::new(false, false);
	let CreateResult { task, .. } = smol::block_on(create(
		request(),
		&fixture,
		&Control::default(),
		&Broken,
		&fixture,
	))
	.unwrap();
	assert_eq!(task.operation.outcome, OperationOutcome::Cancelled);
	assert!(fixture.added.lock().unwrap().is_empty());
	assert_eq!(
		fixture
			.saved
			.lock()
			.unwrap()
			.last()
			.unwrap()
			.operation
			.outcome,
		OperationOutcome::Cancelled
	);
}

impl RecoveryState for Fixture {
	fn load(&self, _: &(), _: &str) -> Result<RecoveryRecord, Problem> {
		Ok(RecoveryRecord {
			relocated_project_roots: Default::default(),
			recorded: None,
			needs_publication: true,
			task: self.saved.lock().unwrap()[0].clone(),
			pending: None,
		})
	}
}

impl RecoveryGit for Fixture {
	async fn reconcile(
		&self,
		participant: &Participant,
		_: &str,
		_: &Control,
	) -> Result<RecoveryObservation, Problem> {
		Ok(RecoveryObservation {
			repository: Repository {
				source: participant.source.clone(),
				common_dir: participant.common_dir.clone(),
				remote: None,
				identity: participant.backing_identity.clone(),
			},
			destination: None,
			operation_in_progress: false,
		})
	}
}

fn assert_identity_failure_settled(task: &Task, fixture: &Fixture) {
	assert_eq!(task.operation.outcome, OperationOutcome::Failed);
	assert_eq!(task.problem.as_ref().unwrap().code, ProblemCode::IoError);
	assert!(
		task.problem
			.as_ref()
			.unwrap()
			.remedy
			.contains("managed writers stopped")
	);
	assert_eq!(task.participants[0].phase, Phase::WorktreeCreated);
	assert_eq!(task.participants[0].worktree_identity, None);
	assert_eq!(task.participants[1].phase, Phase::NotStarted);
	assert_eq!(fixture.added.lock().unwrap().len(), 1);
	let effects = fixture.effects.lock().unwrap();
	let (durable, settled) = effects.last().unwrap();
	assert!(*settled);
	assert_eq!(durable.participants[0].phase, Phase::WorktreeCreated);
	assert_eq!(durable.participants[0].worktree_identity, None);
}

#[test]
fn create_identity_read_failure_retains_success_and_writer_settlement() {
	let mut fixture = Fixture::new(false, false);
	fixture.fail_identity = true;
	let CreateResult { task, .. } = smol::block_on(create(
		request(),
		&fixture,
		&Control::default(),
		&Quiet,
		&fixture,
	))
	.unwrap();
	assert_identity_failure_settled(&task, &fixture);
}

#[test]
fn recovery_identity_read_failure_retains_success_and_writer_settlement() {
	let mut fixture = Fixture::new(false, false);
	// The initial saved admission supplies an unfinished task to recovery.
	smol::block_on(create(
		request(),
		&fixture,
		&Control::default(),
		&Broken,
		&fixture,
	))
	.unwrap();
	fixture.fail_identity = true;
	let result = smol::block_on(recover(
		"feat/test",
		&[],
		&fixture,
		&Control::default(),
		&Quiet,
		&fixture,
	))
	.unwrap();
	assert_identity_failure_settled(&result.task, &fixture);
}

#[test]
fn named_effect_modes_reach_existing_bool_adapter_hook() {
	let fixture = Fixture::new(false, false);
	let CreateResult { task, .. } = smol::block_on(create(
		request(),
		&fixture,
		&Control::default(),
		&Quiet,
		&fixture,
	))
	.unwrap();
	fixture.effects.lock().unwrap().clear();
	fixture
		.record_effect(&(), &task, 0, EffectState::Pending)
		.unwrap();
	fixture
		.record_effect(&(), &task, 0, EffectState::Settled)
		.unwrap();
	let effects = fixture.effects.lock().unwrap();
	assert_eq!(
		effects
			.iter()
			.map(|(_, settled)| *settled)
			.collect::<Vec<_>>(),
		[false, true]
	);
	assert!(
		effects
			.iter()
			.all(|(recorded, _)| recorded.name == task.name)
	);
}

#[test]
fn recovery_publishes_new_attempt_without_erasing_participant_facts() {
	for status in [OperationOutcome::Failed, OperationOutcome::Cancelled] {
		let fixture = Fixture::new(false, false);
		let CreateResult {
			task: mut stored, ..
		} = smol::block_on(create(
			request(),
			&fixture,
			&Control::default(),
			&Broken,
			&fixture,
		))
		.unwrap();
		stored.operation.outcome = status;
		stored.problem = Some(Problem::new(ProblemCode::GitFailed, "previous attempt"));
		stored.participants[0].phase = Phase::Creating;
		*fixture.saved.lock().unwrap() = vec![stored.clone()];
		fixture.effects.lock().unwrap().clear();

		// Stop at the first progress delivery, after publication but before Git work.
		smol::block_on(recover(
			"feat/test",
			&[],
			&fixture,
			&Control::default(),
			&Broken,
			&fixture,
		))
		.unwrap();
		assert!(fixture.added.lock().unwrap().is_empty());

		let saved = fixture.saved.lock().unwrap();
		let effects = fixture.effects.lock().unwrap();
		assert!(effects[0].1);
		for published in [&saved[1], &effects[0].0] {
			assert_eq!(published.operation.outcome, OperationOutcome::Pending);
			assert!(published.problem.is_none());
			assert_eq!(published.participants.len(), stored.participants.len());

			for (actual, expected) in published.participants.iter().zip(&stored.participants) {
				assert_eq!(actual.name, expected.name);
				assert_eq!(actual.source, expected.source);
				assert_eq!(actual.common_dir, expected.common_dir);
				assert_eq!(actual.backing_identity, expected.backing_identity);
				assert_eq!(actual.worktree_identity, expected.worktree_identity);
				assert_eq!(actual.destination, expected.destination);
				assert_eq!(actual.remote, expected.remote);
				assert_eq!(actual.from, expected.from);
				assert_eq!(actual.phase, expected.phase);
			}
		}
	}
}

#[test]
fn optional_pull_failure_continues_but_supervision_uncertainty_does_not() {
	for code in [
		ProblemCode::GitFailed,
		ProblemCode::SupervisionFailed,
		ProblemCode::Cancelled,
	] {
		let mut fixture = Fixture::new(false, false);
		fixture.pull_error = Some(code);
		let mut request = request();
		request.no_fetch = false;
		request.pull = true;
		let result = smol::block_on(create(
			request,
			&fixture,
			&Control::default(),
			&Quiet,
			&fixture,
		))
		.unwrap();
		let effects = fixture.effects.lock().unwrap();
		let (last, settled) = effects.last().unwrap();
		if code == ProblemCode::GitFailed {
			assert_eq!(result.operation.outcome, OperationOutcome::Failed);
			assert_eq!(result.pull_failures.len(), 2);
			assert_eq!(result.task.operation.outcome, OperationOutcome::Succeeded);
			assert!(result.task.problem.is_none());
			assert!(
				result
					.task
					.participants
					.iter()
					.all(|p| p.phase == Phase::Completed)
			);
			assert_eq!(fixture.added.lock().unwrap().len(), 2);
			continue;
		}
		assert_eq!(fixture.added.lock().unwrap().len(), 1);
		assert!(result.pull_failures.is_empty());
		assert_eq!(result.task.participants[1].phase, Phase::NotStarted);
		if code == ProblemCode::SupervisionFailed {
			assert!(!settled);
			assert_eq!(last.participants[0].phase, Phase::Pulling);
		} else {
			assert!(*settled);
			assert_eq!(last.participants[0].phase, Phase::Completed);
			assert_eq!(result.operation.outcome, OperationOutcome::Cancelled);
		}
	}
}

#[test]
fn cancelled_last_pull_keeps_membership_settled_but_invocation_cancelled() {
	let mut fixture = Fixture::new(false, false);
	fixture.pull_error = Some(ProblemCode::Cancelled);
	let mut request = request();
	request.repositories.truncate(1);
	request.no_fetch = false;
	request.pull = true;

	let result = smol::block_on(create(
		request,
		&fixture,
		&Control::default(),
		&Quiet,
		&fixture,
	))
	.unwrap();
	assert_eq!(result.operation.outcome, OperationOutcome::Cancelled);
	assert_eq!(result.problem.unwrap().code, ProblemCode::Cancelled);
	assert_eq!(result.task.operation.outcome, OperationOutcome::Succeeded);
	assert!(result.task.problem.is_none());
	assert_eq!(result.task.participants[0].phase, Phase::Completed);
	assert_eq!(
		fixture
			.saved
			.lock()
			.unwrap()
			.last()
			.unwrap()
			.operation
			.outcome,
		OperationOutcome::Succeeded
	);
}
