use super::{
	DIRECTORIES, Layout, MARKER, PENDING, Workspace, check_entry, checked_marker,
	layout::resolve_tasks, projects::initial_roots,
};
use crate::{Lock, LockMode, canonical_future, documents::publish, exists};
use std::{
	collections::BTreeSet,
	fs::{self, FileType},
	path::{Path, PathBuf},
};
use workyard_core::{Control, Problem, ProblemCode};

/// Task placement and project selection policy requested at initialization.
#[derive(Default)]
pub struct InitOptions<'a> {
	/// Relative tasks path, with placement determined by detached_tasks.
	pub tasks: Option<&'a Path>,
	/// Require an external tasks directory.
	pub detached_tasks: bool,
	/// Limit task participants to discovered repositories.
	pub tracked_only: bool,
	/// Initial discovery roots. Relative paths are anchored at the workspace.
	pub track: &'a [&'a Path],
}

/// Create an initialized workspace, or return its configured tasks directory.
pub fn initialize_workspace(target: &Path, tasks: Option<&Path>) -> Result<PathBuf, Problem> {
	smol::block_on(initialize_workspace_with_control(
		target,
		tasks,
		&Control::default(),
	))
}

/// Initialize a workspace while observing the caller's cancellation policy.
pub async fn initialize_workspace_with_control(
	target: &Path,
	tasks: Option<&Path>,
	control: &Control,
) -> Result<PathBuf, Problem> {
	initialize_workspace_with_options(
		target,
		InitOptions {
			tasks,
			..InitOptions::default()
		},
		control,
	)
	.await
}

/// Initialize a workspace with explicitly selected task and project policies.
pub async fn initialize_workspace_with_options(
	target: &Path,
	options: InitOptions<'_>,
	control: &Control,
) -> Result<PathBuf, Problem> {
	if options.detached_tasks && options.tasks.is_none() {
		return Err(Problem::new(
			ProblemCode::InvalidInput,
			"Supply a detached tasks path.",
		));
	}
	control.check()?;
	let workspace = canonical_future(target)?;
	let requested_roots = initial_roots(&workspace, options.track)?;
	let state = workspace.join(MARKER);
	if !exists(&state)? {
		select_tasks(&workspace, &options)?;
		control.check()?;
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
	let gate = Lock::wait_existing_with_control(&gate_path, mode, control).await?;

	// Recheck under the gate. Another initializer may already have activated its layout.
	if exists(&pending)? {
		let root = select_tasks(&workspace, &options)?;
		control.check()?;
		let roots = requested_roots.unwrap_or_else(|| BTreeSet::from([PathBuf::from("projects")]));
		activate_layout(&workspace, &state, &root, &options, &roots, &gate)?;
		return Ok(root);
	}

	let layout = Layout::read(&state)?;
	let project_conflict = if let Some(roots) = requested_roots {
		layout.project_roots()? != roots
	} else {
		false
	};
	if (options.tasks.is_some() && options.detached_tasks != layout.detached_tasks)
		|| (options.tracked_only && !layout.tracked_only)
		|| project_conflict
	{
		return Err(Problem::new(
			ProblemCode::WorkspaceBindingConflict,
			"Workspace already uses a different tasks or project policy.",
		));
	}
	let requested = options
		.tasks
		.map(|path| resolve_tasks(&workspace, path, options.detached_tasks))
		.transpose()?;
	let root = Workspace::configured_root(&state, requested.as_deref())?;
	control.check()?;
	create_tasks(&root)?;
	gate.verify()?;
	Ok(root)
}

fn select_tasks(workspace: &Path, options: &InitOptions<'_>) -> Result<PathBuf, Problem> {
	let root = resolve_tasks(
		workspace,
		options.tasks.unwrap_or(Path::new("tasks")),
		options.detached_tasks,
	)?;

	for directory in root
		.ancestors()
		.take_while(|directory| *directory != workspace)
	{
		if exists(directory)? && !directory.is_dir() {
			return Err(Problem::new(
				ProblemCode::InvalidInput,
				format!("Tasks path is not a directory: {directory:?}."),
			));
		}
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
	options: &InitOptions<'_>,
	roots: &BTreeSet<PathBuf>,
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
	let tasks = if options.detached_tasks {
		options
			.tasks
			.expect("detached initialization requires an explicit tasks path")
	} else {
		root.strip_prefix(workspace)
			.expect("validated tasks directory")
	};
	let layout = Layout::new(
		workspace,
		tasks,
		options.detached_tasks,
		options.tracked_only,
		roots,
	);
	publish(&pending, &layout)?;
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
