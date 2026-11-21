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
		let workspace = temporary.path().join("workspace");
		let root = workyard_state::initialize_workspace(&workspace, None).unwrap();
		let state = workspace.join(".workyard").canonicalize().unwrap();
		let store = Store::open_existing(&state, Some(&root)).unwrap();
		Self {
			store,
			root,
			state,
			temporary,
		}
	}
}
