use std::path::{Path, PathBuf};
use workyard_core::Task;
use workyard_state::Store;

pub fn task(root: &Path, state: &Path, name: &str) -> Task {
	Task {
		generation: 0,
		removal: None,
		name: name.into(),
		root: root.into(),
		state_dir: state.into(),
		participants: Vec::new(),
		checkout: false,
		no_fetch: true,
		pull: false,
		lifecycle: workyard_core::TaskLifecycle::Active,
		operation: workyard_core::Operation::create(),
		problem: None,
	}
}

pub struct Workspace {
	pub store: Store,
	pub root: PathBuf,
	pub state: PathBuf,
	// Owns the directory lifetime even in suites that do not inspect its path.
	#[allow(dead_code)]
	pub temporary: tempfile::TempDir,
}

impl Workspace {
	pub fn new() -> Self {
		let temporary = tempfile::tempdir().unwrap();
		let root = temporary.path().join("root");
		let state = root.join(".workyard");
		let store = Store::open(&root, &state).unwrap();
		Self {
			store,
			root,
			state,
			temporary,
		}
	}
}
