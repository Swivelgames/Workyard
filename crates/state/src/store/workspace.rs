use super::Store;
use crate::documents::{publish, read_versioned};
use crate::records::WorkspaceRecord;
use crate::{Lock, canonical_future, exists};
use std::{fs, path::Path};
use workyard_core::Problem;

impl Store {
	pub fn open(root: &Path, state: &Path) -> Result<Self, Problem> {
		validate_default(root, state)?;
		fs::create_dir_all(root).map_err(|e| Problem::io("Create tasks root", e))?;
		fs::create_dir_all(state).map_err(|e| Problem::io("Create state directory", e))?;
		let root = canonical_future(root)?;
		let state = canonical_future(state)?;
		let gate = state.join("workspace.lock");
		bootstrap(&root, &state, &gate)?;

		let workspace = Lock::acquire(&gate, true)?;
		let store = Self {
			root,
			state,
			workspace,
		};
		store.verify()?;
		validate_default(&store.root, &store.state)?;
		for directory in ["tasks", "locks", "evidence"] {
			fs::create_dir_all(store.state.join(directory))
				.map_err(|e| Problem::io("Create state subdirectory", e))?;
		}

		Ok(store)
	}

	pub fn verify(&self) -> Result<(), Problem> {
		self.workspace.verify()?;
		let record: WorkspaceRecord = read_versioned(&self.state.join("workspace.json"))?;
		if record.tasks_root.decode()? != self.root || record.state_dir.decode()? != self.state {
			return Err(Problem::new(
				workyard_core::ProblemCode::WorkspaceBindingConflict,
				"State directory is bound to a different workspace or location.",
			)
			.remedy(
				"Use the recorded tasks root and state directory; do not hand-rebind active state.",
			));
		}

		Ok(())
	}
}

fn validate_default(root: &Path, state: &Path) -> Result<(), Problem> {
	let root = canonical_future(root)?;
	let state = canonical_future(state)?;
	let default = root.join(".workyard");
	if canonical_future(&default)? == state || !exists(&default)? {
		return Ok(());
	}

	let _gate = Lock::acquire(&default.join("workspace.lock"), true)?;
	let record: WorkspaceRecord = read_versioned(&default.join("workspace.json"))?;
	if canonical_future(&record.tasks_root.decode()?)? == root {
		return Err(Problem::new(
			workyard_core::ProblemCode::WorkspaceBindingConflict,
			"The tasks root already has a default state directory.",
		));
	}

	Ok(())
}

/// Publish the binding under exclusive protection, then release it before the
/// long-lived shared workspace gate is acquired.
fn bootstrap(root: &Path, state: &Path, gate: &Path) -> Result<(), Problem> {
	let manifest = state.join("workspace.json");
	if exists(&manifest)? {
		return Ok(());
	}

	let bootstrap = Lock::acquire(gate, false)?;
	bootstrap.verify()?;
	if exists(&manifest)? {
		return Ok(());
	}

	// An interrupted bootstrap is not permission to reset an occupied store.
	let entries = fs::read_dir(state)
		.map_err(|e| Problem::io("Read state directory", e))?
		.collect::<Result<Vec<_>, _>>()
		.map_err(|e| Problem::io("Read state entry", e))?;
	let occupied = entries
		.iter()
		.any(|entry| entry.file_name() != "workspace.lock");
	if occupied {
		let problem = Problem::new(
			workyard_core::ProblemCode::DamagedState,
			"State directory has contents but no workspace.json.",
		);
		return Err(problem
			.remedy("Inspect the selected state directory; select a fresh --state-dir for unrelated work. Do not reset existing evidence."));
	}

	let record = WorkspaceRecord {
		version: 1,
		tasks_root: root.into(),
		state_dir: state.into(),
	};
	publish(&manifest, &record)
}
