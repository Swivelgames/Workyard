//! Each edit merges into the latest manifest under its own short-lived lock.
use super::Layout;
use crate::{Lock, Reader, Store, documents};
use std::{
	collections::BTreeSet,
	path::{Component, Path, PathBuf},
};
use workyard_core::{Control, Problem, ProblemCode};

/// Requested change to the project's discovery roots.
#[derive(Clone, Copy)]
pub enum RootEdit {
	/// Add the supplied roots to the project catalog configuration.
	Track,
	/// Remove the supplied roots from the project catalog configuration.
	Untrack,
}

impl Reader {
	/// Whether explicit participants must match a discovered project.
	pub fn tracked_only(&self) -> Result<bool, Problem> {
		Ok(Layout::read(self.state())?.tracked_only)
	}

	/// Absolute selection to retain for a deferred project registration.
	pub fn project_registration_path(&self, path: &Path) -> Result<PathBuf, Problem> {
		let workspace = self.state().parent().expect("workspace marker");
		Ok(workspace.join(registration(workspace, path)?))
	}

	/// Read the workspace's configured discovery roots.
	pub fn project_roots(&self) -> Result<Vec<PathBuf>, Problem> {
		let workspace = self.state().parent().expect("workspace marker");
		Ok(Layout::read(self.state())?
			.located_roots(workspace)?
			.into_iter()
			.map(|root| workspace.join(root))
			.collect())
	}
}

impl Store {
	/// Record the verified location of the active workspace.
	pub async fn remember_workspace(&self, control: &Control) -> Result<bool, Problem> {
		let state = self.state_dir();
		let workspace = state.parent().expect("workspace marker");
		control.check()?;
		if Layout::read(state)?.is_at(workspace)? {
			return Ok(false);
		}
		let lock = Lock::wait(&state.join("locks/projects.lock"), control).await?;
		self.verify()?;
		let mut layout = Layout::read(state)?;
		if !layout.update_workspace(workspace)? {
			return Ok(false);
		}
		lock.verify()?;
		control.check()?;
		documents::publish(&state.join("workspace.json"), &layout)?;
		Ok(true)
	}

	/// Apply the requested root changes to workspace configuration.
	pub async fn edit_project_roots(
		&self,
		edit: RootEdit,
		paths: &[PathBuf],
		control: &Control,
	) -> Result<Vec<PathBuf>, Problem> {
		if paths.is_empty() {
			return Ok(Vec::new());
		}
		let workspace = self.state_dir().parent().expect("workspace marker");
		let lock = Lock::wait(&self.state_dir().join("locks/projects.lock"), control).await?;
		self.verify()?;
		let mut layout = Layout::read(self.state_dir())?;
		let roots = paths
			.iter()
			.map(|path| edit.registration(workspace, path))
			.collect::<Result<BTreeSet<_>, _>>()?;
		// Evaluate edits against the same roots a project listing would show here.
		layout.update_workspace(workspace)?;
		let mut registered = layout.project_roots()?;
		let mut changed = Vec::new();

		for root in roots {
			let matches: Vec<_> = registered
				.iter()
				.filter(|stored| {
					**stored == root
						|| (stored.components().any(|part| part == Component::ParentDir)
							&& registration(workspace, &workspace.join(stored))
								.is_ok_and(|path| path == root))
				})
				.cloned()
				.collect();
			match edit {
				RootEdit::Track if matches.is_empty() => {
					registered.insert(root.clone());
				}
				RootEdit::Untrack if !matches.is_empty() => {
					for stored in matches {
						registered.remove(&stored);
					}
				}
				_ => continue,
			}
			changed.push(workspace.join(root));
		}
		if !changed.is_empty() {
			control.check()?;
			lock.verify()?;
			layout.set_project_roots(&registered);
			documents::publish(&self.state_dir().join("workspace.json"), &layout)?;
		}
		Ok(changed)
	}
}

pub(super) fn initial_roots(
	workspace: &Path,
	paths: &[&Path],
) -> Result<Option<BTreeSet<PathBuf>>, Problem> {
	if paths.is_empty() {
		return Ok(None);
	}

	let roots = paths
		.iter()
		.map(|&path| {
			if path.is_absolute() {
				return RootEdit::Track.registration(workspace, path);
			}
			if path.as_os_str().is_empty()
				|| path
					.components()
					.any(|part| matches!(part, Component::RootDir | Component::Prefix(_)))
			{
				return Err(Problem::new(
					ProblemCode::InvalidInput,
					"Supply a relative or absolute project path.",
				));
			}

			let resolved = crate::canonical_future(&workspace.join(path))?;
			validate_track_path(&resolved)?;
			Ok(path.to_owned())
		})
		.collect::<Result<BTreeSet<_>, _>>()?;
	Ok(Some(roots))
}

impl RootEdit {
	fn registration(self, workspace: &Path, path: &Path) -> Result<PathBuf, Problem> {
		if matches!(self, Self::Track) {
			validate_track_path(path)?;
		}
		registration(workspace, path)
	}
}

fn validate_track_path(path: &Path) -> Result<(), Problem> {
	match std::fs::metadata(path) {
		Ok(meta) if !meta.is_dir() => Err(Problem::new(
			ProblemCode::InvalidInput,
			format!("Project root {path:?} must be a directory."),
		)),
		Err(error) if error.kind() != std::io::ErrorKind::NotFound => {
			Err(Problem::io(format!("Inspect project root {path:?}"), error))
		}
		_ => Ok(()),
	}
}

// Preserve the registered path as a discovery source, including a directory symlink.
// Resolve parent traversals with filesystem semantics before storing a portable spelling.
fn registration(workspace: &Path, path: &Path) -> Result<PathBuf, Problem> {
	if path.as_os_str().is_empty() {
		return Err(Problem::new(
			workyard_core::ProblemCode::InvalidInput,
			"Project root cannot be empty.",
		));
	}
	let absolute =
		std::path::absolute(path).map_err(|error| Problem::io("Resolve project root", error))?;
	// Canonical parents also put Windows drive paths in the workspace marker's native form.
	let inside = absolute.starts_with(workspace)
		&& !absolute
			.components()
			.any(|part| part == std::path::Component::ParentDir);
	let absolute = match (absolute.parent(), absolute.file_name()) {
		_ if inside => absolute,
		(Some(parent), Some(name)) => crate::canonical_future(parent)?.join(name),
		_ => crate::canonical_future(&absolute)?,
	};
	let relative = absolute.strip_prefix(workspace).unwrap_or(&absolute);
	Ok(if relative.as_os_str().is_empty() {
		PathBuf::from(".")
	} else {
		relative.to_owned()
	})
}
