//! Each edit merges into the latest manifest under its own short-lived lock.
use super::Layout;
use crate::{Lock, Reader, Store, documents};
use std::{
	collections::BTreeSet,
	path::{Path, PathBuf},
};
use workyard_core::{Control, Problem, ProblemCode};

#[derive(Clone, Copy)]
pub enum RootEdit {
	Track,
	Untrack,
}

impl Reader {
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
			let updated = match edit {
				RootEdit::Track => registered.insert(root.clone()),
				RootEdit::Untrack => registered.remove(&root),
			};
			if updated {
				changed.push(workspace.join(root));
			}
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

impl RootEdit {
	fn registration(self, workspace: &Path, path: &Path) -> Result<PathBuf, Problem> {
		if matches!(self, Self::Track) {
			match std::fs::metadata(path) {
				Ok(meta) if !meta.is_dir() => {
					return Err(Problem::new(
						ProblemCode::InvalidInput,
						format!("Project root {path:?} must be a directory."),
					));
				}
				Err(error) if error.kind() != std::io::ErrorKind::NotFound => {
					return Err(Problem::io(format!("Inspect project root {path:?}"), error));
				}
				_ => {}
			}
		}
		registration(workspace, path)
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
