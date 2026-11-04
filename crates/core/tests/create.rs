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
}

impl Fixture {
	fn new(cancel_after_add: bool, fail_publish: bool) -> Self {
		Self {
			cancel_after_add,
			cancelled: Default::default(),
			added: Default::default(),
			saved: Default::default(),
			fail_publish,
		}
	}
}

impl Git for Fixture {
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
		Ok(())
	}
}

impl State for Fixture {
	type Admission = ();
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
fn cancellation_racing_success_records_known_worktree_and_stops_next_repo() {
	let fixture = Fixture::new(true, false);
	let cancelled = fixture.cancelled.clone();
	let control = Control::new(move || cancelled.load(Ordering::SeqCst));
	let (task, _) =
		smol::block_on(create(request(), &fixture, &control, &Quiet, &fixture)).unwrap();
	assert_eq!(task.status, TaskStatus::Cancelled);
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
	let (task, _) = smol::block_on(create(
		request(),
		&fixture,
		&Control::default(),
		&Quiet,
		&fixture,
	))
	.unwrap();
	assert_eq!(task.status, TaskStatus::Failed);
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
	let (task, _) = smol::block_on(create(
		request(),
		&fixture,
		&Control::default(),
		&Broken,
		&fixture,
	))
	.unwrap();
	assert_eq!(task.status, TaskStatus::Cancelled);
	assert!(fixture.added.lock().unwrap().is_empty());
	assert_eq!(
		fixture.saved.lock().unwrap().last().unwrap().status,
		TaskStatus::Cancelled
	);
}
