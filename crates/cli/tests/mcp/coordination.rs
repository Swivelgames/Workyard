//! Synchronize executable tests with real Git effects and their durable evidence.
use serde_json::Value;
use std::{
	collections::BTreeMap,
	fs::{self, File},
	path::{Path, PathBuf},
	thread,
	time::{Duration, Instant},
};
use workyard_core::{Observer, Problem, ProblemCode, RemovalObserver};

pub(super) fn hold_fetch_lock(repository: &Path) -> File {
	hold_repository_lock(repository, "fetch.lock")
}

pub(super) fn hold_relationship_lock(repository: &Path) -> File {
	hold_repository_lock(repository, "relationships.lock")
}

fn hold_repository_lock(repository: &Path, name: &str) -> File {
	let directory = repository.join(".git/workyard");
	fs::create_dir_all(&directory).unwrap();
	let lock = File::options()
		.create(true)
		.truncate(false)
		.read(true)
		.write(true)
		.open(directory.join(name))
		.unwrap();
	lock.try_lock().unwrap();
	lock
}

pub(super) fn wait_for_effect(workspace: &Path, task: &str, index: usize, phase: &str) {
	let effects = workspace
		.join(".workyard/evidence")
		.join(workyard_state::task_filename(task))
		.with_extension("effects");
	wait_until(phase, || {
		let bytes = match fs::read(&effects) {
			Ok(bytes) => bytes,
			Err(error) if error.kind() == std::io::ErrorKind::NotFound => return false,
			Err(error) => panic!("read task evidence: {error}"),
		};
		let record: Value = serde_json::from_slice(&bytes).unwrap();
		record["pending"] == index && record["task"]["participants"][index]["phase"] == phase
	});
}

pub(super) fn wait_until(description: &str, mut ready: impl FnMut() -> bool) {
	let deadline = Instant::now() + Duration::from_secs(15);
	while !ready() {
		assert!(
			Instant::now() < deadline,
			"timed out waiting for {description}"
		);
		thread::sleep(Duration::from_millis(10));
	}
}

pub(super) struct StopAtPhase(pub(super) &'static str);

impl RemovalObserver for StopAtPhase {}

impl Observer for StopAtPhase {
	async fn phase(&self, _: &str, _: &str, phase: &str) -> Result<(), Problem> {
		if phase == self.0 {
			return Err(Problem::new(
				ProblemCode::Cancelled,
				format!("Stop at {phase}"),
			));
		}
		Ok(())
	}
}

pub(super) fn task_documents(state: &Path) -> BTreeMap<PathBuf, Vec<u8>> {
	let mut documents = BTreeMap::new();
	for directory in ["tasks", "evidence"] {
		for entry in fs::read_dir(state.join(directory)).unwrap() {
			let path = entry.unwrap().path();
			let bytes = fs::read(&path).unwrap();
			documents.insert(path, bytes);
		}
	}
	documents
}
