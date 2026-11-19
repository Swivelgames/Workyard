use super::{MARKER, PENDING, check_entry};
use crate::{canonical_future, damage::damaged, documents, exists, path::StoredPath};
use serde::{Deserialize, Serialize};
use std::{
	collections::BTreeSet,
	fs::FileType,
	path::{Component, Path, PathBuf},
};
use workyard_core::{Problem, ProblemCode};

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Layout {
	version: u32,
	workspace_root: StoredPath,
	tasks_dir: StoredPath,
	project_roots: Vec<StoredPath>,
}

impl Layout {
	pub fn new(workspace: &Path, tasks: &Path) -> Self {
		Self {
			version: 4,
			workspace_root: workspace.into(),
			tasks_dir: tasks.into(),
			project_roots: vec![Path::new("projects").into()],
		}
	}

	pub fn read(state: &Path) -> Result<Self, Problem> {
		let path = state.join("workspace.json");
		if !exists(&path)? && exists(&state.join(PENDING))? {
			return Err(damaged("Workspace initialization is incomplete.")
				.remedy("Run workyard init in this workspace to finish."));
		}
		check_entry(&path, FileType::is_file)?;
		let layout: Self = documents::read_supported(&path, &[4])?;
		if !layout.workspace_root.decode()?.is_absolute() {
			return Err(damaged("The recorded workspace location must be absolute."));
		}
		Ok(layout)
	}

	pub fn is_at(&self, workspace: &Path) -> Result<bool, Problem> {
		Ok(self.workspace_root.registration()? == workspace)
	}

	pub fn update_workspace(&mut self, workspace: &Path) -> Result<bool, Problem> {
		let previous = self.workspace_root.registration()?;
		if previous == workspace {
			return Ok(false);
		}
		let roots = self.located_roots(workspace)?;
		self.set_project_roots(&roots);
		self.workspace_root = workspace.into();
		Ok(true)
	}

	/// Roots under the workspace follow it. An explicitly tracked root left
	/// behind remains discoverable; the default projects directory follows init.
	pub fn located_roots(&self, workspace: &Path) -> Result<BTreeSet<PathBuf>, Problem> {
		let previous = self.workspace_root.registration()?;
		let roots = self.project_roots()?;
		if previous == workspace {
			return Ok(roots);
		}
		let mut located = BTreeSet::new();
		for root in roots {
			let current = workspace.join(&root);
			let old = previous.join(&root);
			if root.is_absolute() || root == Path::new("projects") || crate::exists(&current)? {
				located.insert(root);
			} else if crate::exists(&old)? {
				located.insert(old);
			} else {
				located.insert(root);
			}
		}
		Ok(located)
	}

	pub fn project_roots(&self) -> Result<BTreeSet<PathBuf>, Problem> {
		self.project_roots
			.iter()
			.map(|stored| {
				let path = stored.decode()?;
				if path.as_os_str().is_empty()
					|| path.as_os_str().as_encoded_bytes().contains(&0)
					|| path.components().any(|c| c == Component::ParentDir)
					|| (!path.is_absolute()
						&& path
							.components()
							.any(|c| matches!(c, Component::RootDir | Component::Prefix(_))))
				{
					return Err(damaged("Invalid project root in workspace.json."));
				}
				Ok(path)
			})
			.collect()
	}

	pub fn set_project_roots(&mut self, roots: &BTreeSet<PathBuf>) {
		self.project_roots = roots.iter().map(|root| root.as_path().into()).collect();
	}

	pub fn tasks_root(&self, state: &Path) -> Result<PathBuf, Problem> {
		let workspace = state.parent().expect("validated workspace marker");
		resolve_tasks(workspace, &self.tasks_dir.decode()?)
	}
}

pub(super) fn resolve_tasks(workspace: &Path, tasks: &Path) -> Result<PathBuf, Problem> {
	if tasks.is_absolute()
		|| tasks
			.components()
			.any(|part| matches!(part, Component::Prefix(_) | Component::RootDir))
	{
		return Err(Problem::new(
			ProblemCode::InvalidInput,
			"The tasks directory must be relative to the workspace.",
		));
	}
	let root = canonical_future(&workspace.join(tasks))?;
	let relative = root.strip_prefix(workspace).ok();
	let allowed = relative.is_some_and(|path| {
		!path.as_os_str().is_empty()
			&& !path
				.components()
				.any(|part| part.as_os_str().eq_ignore_ascii_case(MARKER))
	});
	if !allowed {
		return Err(Problem::new(
			ProblemCode::PathOverlap,
			"The tasks directory must be beneath the workspace and outside .workyard.",
		));
	}

	Ok(root)
}
