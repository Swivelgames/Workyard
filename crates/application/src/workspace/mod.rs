//! Workspace selection shared by mutations, observations and shell completion.
use std::path::{Path, PathBuf};
use workyard_core::{Problem, ProblemCode};

pub(crate) fn required(workspace: Option<&Path>) -> Result<PathBuf, Problem> {
	let cwd =
		std::env::current_dir().map_err(|error| Problem::io("Read current directory", error))?;
	workyard_state::find_state(&cwd, workspace)?.ok_or_else(|| {
		Problem::new(
			ProblemCode::WorkspaceNotFound,
			format!("No {} workspace found.", crate::identity::Identity::name()),
		)
		.remedy(crate::guidance::workspace::init_guidance(workspace))
	})
}

pub(crate) fn reader(
	cwd: &Path,
	workspace: Option<&Path>,
) -> Result<Option<workyard_state::Reader>, Problem> {
	workyard_state::find_state(cwd, workspace)?
		.map(|state| workyard_state::Reader::open(&state, None))
		.transpose()
}
