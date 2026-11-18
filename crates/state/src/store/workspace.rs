use super::Store;
use crate::damage::damaged;
use crate::documents::publish;
use crate::records::WorkspaceRecord;
use crate::{Lock, canonical_future, exists};
use std::{fs, path::Path};
use workyard_core::{Problem, ProblemCode};

impl Store {
	/// Existing-task operations never create a workspace or repair its structure.
	pub fn open_existing(state: &Path, requested_root: Option<&Path>) -> Result<Self, Problem> {
		if !exists(state)? {
			return Err(Problem::new(ProblemCode::DamagedState, "Authoritative state does not exist.")
				.remedy("Select the existing --state-dir, or run from beneath its .workyard ancestor. Recovery cannot initialize state."));
		}
		let state = canonical_future(state)?;
		for name in [
			"workspace.json",
			"workspace.lock",
			"tasks",
			"locks",
			"evidence",
		] {
			if !exists(&state.join(name))? {
				return Err(damaged(format!(
					"Existing state is missing {name}. Preserve it and restore its authoritative metadata."
				)));
			}
		}
		let workspace =
			crate::Lock::acquire_mode(&state.join("workspace.lock"), crate::LockMode::Shared)?;
		let record: crate::records::WorkspaceRecord =
			WorkspaceRecord::read(&state.join("workspace.json"))?;
		let root = canonical_future(&record.tasks_root.decode()?)?;
		if let Some(requested) = requested_root
			&& canonical_future(requested)? != root
		{
			return Err(Problem::new(
				ProblemCode::WorkspaceBindingConflict,
				"Requested tasks root differs from the selected state.",
			));
		}
		let store = Self {
			root,
			state,
			workspace,
		};
		store.verify()?;
		validate_default(&store.root, &store.state)?;
		Ok(store)
	}

	pub fn open(root: &Path, state: &Path) -> Result<Self, Problem> {
		validate_default(root, state)?;
		fs::create_dir_all(root).map_err(|e| Problem::io("Create tasks root", e))?;
		fs::create_dir_all(state).map_err(|e| Problem::io("Create state directory", e))?;
		let root = canonical_future(root)?;
		let state = canonical_future(state)?;
		let gate = state.join("workspace.lock");
		bootstrap(&root, &state, &gate)?;

		let workspace = Lock::acquire_mode(&gate, crate::LockMode::Shared)?;
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
		let record: WorkspaceRecord = WorkspaceRecord::read(&self.state.join("workspace.json"))?;
		crate::relocation::validate_mutation_binding(&self.state, &record)?;

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

pub(super) fn validate_default(root: &Path, state: &Path) -> Result<(), Problem> {
	let root = canonical_future(root)?;
	let state = canonical_future(state)?;
	let default = root.join(".workyard");
	if canonical_future(&default)? == state || !exists(&default)? {
		return Ok(());
	}

	let _gate = Lock::acquire_mode(&default.join("workspace.lock"), crate::LockMode::Shared)?;
	let record: WorkspaceRecord = WorkspaceRecord::read(&default.join("workspace.json"))?;
	if canonical_future(&record.tasks_root.decode()?)? == root {
		if record
			.authority
			.as_deref()
			.is_some_and(|authority| authority != "active")
			&& record
				.relocation_destination
				.as_ref()
				.map(|path| path.decode())
				.transpose()?
				.as_deref() == Some(state.as_path())
		{
			let selected = WorkspaceRecord::read(&state.join("workspace.json"))?;
			if selected.authority.as_deref() == Some("active")
				&& selected.relocation.is_some()
				&& selected.relocation == record.relocation
				&& selected.state_dir.decode()? == state
				&& selected.tasks_root.decode()? == root
			{
				return Ok(());
			}
		}
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

	let bootstrap = Lock::acquire_mode(gate, crate::LockMode::Exclusive)?;
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
		relocation: None,
		authority: None,
		relocation_source: None,
		relocation_destination: None,
		tasks_root: root.into(),
		state_dir: state.into(),
	};
	publish(&manifest, &record)
}
