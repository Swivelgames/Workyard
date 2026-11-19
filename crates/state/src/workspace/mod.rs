//! Fixed workspace marker, relative task placement and read-only admission.
mod initialize;
mod layout;
mod projects;
pub use projects::RootEdit;

pub use initialize::initialize_workspace;
use layout::Layout;

use crate::{Lock, canonical_future, damage::damaged};
use std::{
	fs::{self, FileType},
	path::{Path, PathBuf},
};
use workyard_core::{Problem, ProblemCode};

const MARKER: &str = ".workyard";
const PENDING: &str = "initializing.json";
const DIRECTORIES: &[&str] = &["tasks", "evidence", "locks"];

pub(crate) struct Workspace {
	pub root: PathBuf,
	pub state: PathBuf,
	pub gate: Lock,
}

impl Workspace {
	pub fn open(state: &Path, requested_root: Option<&Path>) -> Result<Self, Problem> {
		let state = checked_marker(state)?;
		let gate = read_gate(&state)?;
		let root = Self::configured_root(&state, requested_root)?;
		gate.verify()?;
		Ok(Self { root, state, gate })
	}

	fn configured_root(state: &Path, requested_root: Option<&Path>) -> Result<PathBuf, Problem> {
		let root = Layout::read(state)?.tasks_root(state)?;
		if let Some(requested) = requested_root
			&& canonical_future(requested)? != root
		{
			return Err(Problem::new(
				ProblemCode::WorkspaceBindingConflict,
				format!(
					"Workspace already uses tasks directory {root:?}. Requested {requested:?}."
				),
			));
		}
		check_directories(state)?;
		Ok(root)
	}

	pub fn verify(state: &Path, root: &Path, gate: &Lock) -> Result<(), Problem> {
		gate.verify()?;
		checked_marker(state)?;
		if Layout::read(state)?.tasks_root(state)? != root {
			return Err(damaged(
				"Workspace task directory changed during the operation.",
			));
		}
		Ok(())
	}
}

pub(crate) fn checked_marker(state: &Path) -> Result<PathBuf, Problem> {
	if state.file_name() != Some(MARKER.as_ref()) {
		return Err(Problem::new(
			ProblemCode::InvalidInput,
			"Workspace state must be in its .workyard directory.",
		));
	}
	check_entry(state, FileType::is_dir)?;
	canonical_future(state)
}

fn check_directories(state: &Path) -> Result<(), Problem> {
	for name in DIRECTORIES {
		check_entry(&state.join(name), FileType::is_dir)?;
	}
	Ok(())
}

fn read_gate(state: &Path) -> Result<Lock, Problem> {
	let path = state.join("workspace.lock");
	check_entry(&path, FileType::is_file)?;
	Lock::read_existing(&path)
}

/// State entries have fixed roles; a symlink could redirect reads or writes into another store.
fn check_entry(path: &Path, expected: fn(&FileType) -> bool) -> Result<(), Problem> {
	let metadata = fs::symlink_metadata(path).map_err(|error| {
		if error.kind() == std::io::ErrorKind::NotFound {
			return damaged(format!("Missing workspace entry {path:?}."));
		}
		Problem::io(format!("Inspect workspace entry {path:?}"), error)
	})?;
	if !expected(&metadata.file_type()) {
		return Err(damaged(format!(
			"Unexpected file type at workspace entry {path:?}."
		)));
	}
	Ok(())
}
