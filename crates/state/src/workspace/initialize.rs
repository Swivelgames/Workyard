use super::{
	DIRECTORIES, Layout, MARKER, PENDING, Workspace, check_entry, checked_marker,
	layout::resolve_tasks,
};
use crate::{Lock, LockMode, canonical_future, documents::publish, exists};
use std::{
	fs::{self, FileType},
	path::{Path, PathBuf},
};
use workyard_core::{Problem, ProblemCode};

/// Create an initialized workspace, or return its configured tasks directory.
pub fn initialize_workspace(target: &Path, tasks: Option<&Path>) -> Result<PathBuf, Problem> {
	let workspace = canonical_future(target)?;
	let state = workspace.join(MARKER);
	if !exists(&state)? {
		select_tasks(&workspace, tasks)?;
		fs::create_dir_all(&workspace)
			.map_err(|error| Problem::io("Create workspace directory", error))?;
		stage_marker(&workspace, &state)?;
	}

	checked_marker(&state)?;
	let pending = state.join(PENDING);
	let gate_path = state.join("workspace.lock");
	check_entry(&gate_path, FileType::is_file)?;
	let mode = if exists(&pending)? {
		LockMode::Exclusive
	} else {
		LockMode::Shared
	};
	let gate = Lock::wait_existing(&gate_path, mode)?;

	// Recheck under the gate. Another initializer may already have activated its layout.
	if exists(&pending)? {
		let root = select_tasks(&workspace, tasks)?;
		activate_layout(&workspace, &state, &root, &gate)?;
		return Ok(root);
	}

	let requested = tasks
		.map(|path| resolve_tasks(&workspace, path))
		.transpose()?;
	let root = Workspace::configured_root(&state, requested.as_deref())?;
	create_tasks(&root)?;
	gate.verify()?;
	Ok(root)
}

fn select_tasks(workspace: &Path, tasks: Option<&Path>) -> Result<PathBuf, Problem> {
	let root = resolve_tasks(workspace, tasks.unwrap_or(Path::new("tasks")))?;
	if exists(&root)? && !root.is_dir() {
		return Err(Problem::new(
			ProblemCode::InvalidInput,
			format!("Tasks path is not a directory: {root:?}."),
		));
	}
	for directory in root
		.ancestors()
		.take_while(|directory| *directory != workspace)
	{
		if exists(&directory.join(MARKER))? {
			return Err(Problem::new(
				ProblemCode::PathOverlap,
				format!("The tasks directory crosses a nested workspace at {directory:?}."),
			));
		}
	}
	Ok(root)
}

fn activate_layout(
	workspace: &Path,
	state: &Path,
	root: &Path,
	gate: &Lock,
) -> Result<(), Problem> {
	let pending = state.join(PENDING);
	let manifest = state.join("workspace.json");
	check_entry(&pending, FileType::is_file)?;
	super::check_directories(state)?;
	if exists(&manifest)? {
		return Err(crate::damage::damaged(
			"Workspace contains both a published and a pending layout.",
		));
	}

	create_tasks(root)?;
	let relative = root
		.strip_prefix(workspace)
		.expect("validated tasks directory");
	publish(&pending, &Layout::new(workspace, relative))?;
	gate.verify()?;
	fs::rename(pending, manifest).map_err(|error| Problem::io("Activate workspace layout", error))
}

fn create_tasks(root: &Path) -> Result<(), Problem> {
	fs::create_dir_all(root)
		.map_err(|error| Problem::io(format!("Create tasks directory {root:?}"), error))
}

fn stage_marker(workspace: &Path, state: &Path) -> Result<(), Problem> {
	// Install the gate and its directories together. The pending file is renamed
	// to workspace.json only after the winning initializer establishes the tasks dir.
	let staging = tempfile::Builder::new()
		.prefix(".workyard-init-")
		.tempdir_in(workspace)
		.map_err(|error| Problem::io("Stage workspace marker", error))?;
	for name in DIRECTORIES {
		fs::create_dir(staging.path().join(name))
			.map_err(|error| Problem::io("Stage workspace directory", error))?;
	}
	for name in ["workspace.lock", PENDING] {
		fs::File::create_new(staging.path().join(name))
			.map_err(|error| Problem::io("Stage workspace files", error))?;
	}

	match fs::rename(staging.path(), state) {
		Ok(()) => Ok(()),
		Err(_) if exists(state)? => Ok(()),
		Err(error) => Err(Problem::io("Publish workspace marker", error)),
	}
}
