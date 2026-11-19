use std::path::{Component, Path, PathBuf};
use workyard_core::{Problem, ProblemCode, projects::Catalog};

pub(crate) struct ResolvedProject {
	pub path: PathBuf,
	pub(super) identity: Option<String>,
}

pub(crate) fn is_explicit(path: &Path) -> bool {
	path.is_absolute()
		|| path
			.components()
			.any(|part| !matches!(part, Component::Normal(_)))
}

pub(crate) fn resolve_selector(
	catalog: &Catalog,
	cwd: &Path,
	selector: &Path,
	grouped: bool,
) -> Result<ResolvedProject, Problem> {
	if selector.as_os_str().is_empty() {
		return Err(Problem::new(
			ProblemCode::InvalidInput,
			"Project selector cannot be empty.",
		));
	}
	// A grouped repository argument historically accepted paths relative to CWD.
	if is_explicit(selector) || (grouped && cwd.join(selector).is_dir()) {
		return Ok(ResolvedProject {
			path: cwd.join(selector),
			identity: None,
		});
	}
	match catalog.resolve(selector) {
		Ok(project) => Ok(ResolvedProject {
			path: project.path.clone(),
			identity: Some(project.identity.clone()),
		}),
		Err(problem) if grouped && problem.code == ProblemCode::ProjectUnknown => {
			Ok(ResolvedProject {
				path: cwd.join(selector),
				identity: None,
			})
		}
		Err(problem) => Err(problem),
	}
}
