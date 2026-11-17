use std::path::{Path, PathBuf};
use workyard_core::Problem;

/// Resolved placement for existing state; never initializes a workspace.
pub(crate) struct Existing {
	pub root: Option<PathBuf>,
	pub state: PathBuf,
}

pub(crate) struct NewWorkspace {
	pub root: PathBuf,
	pub state: PathBuf,
}

pub(crate) fn existing(root: Option<&Path>, explicit: Option<&Path>) -> Result<Existing, Problem> {
	let cwd =
		std::env::current_dir().map_err(|error| Problem::io("Read current directory", error))?;
	let state = workyard_state::discover_state(root.unwrap_or(&cwd), explicit)?;
	Ok(Existing {
		root: root.map(Path::to_path_buf),
		state,
	})
}

/// New-workspace placement deliberately permits paths that do not exist yet.
pub(crate) fn new_workspace(
	root: Option<&Path>,
	explicit: Option<&Path>,
) -> Result<NewWorkspace, Problem> {
	let cwd =
		std::env::current_dir().map_err(|error| Problem::io("Read current directory", error))?;
	let root = workyard_state::canonical_future(root.unwrap_or(&cwd))?;
	let state = workyard_state::canonical_future(
		&explicit.map_or_else(|| root.join(".workyard"), Path::to_path_buf),
	)?;
	Ok(NewWorkspace { root, state })
}
