use std::path::Path;
use workyard_core::Problem;

/// Existing-only optional read access; never initializes or admits mutations.
pub(crate) fn optional(
	cwd: &Path,
	root: Option<&Path>,
	explicit: Option<&Path>,
) -> Result<Option<workyard_state::Reader>, Problem> {
	let Some(state) = workyard_state::find_state(root.unwrap_or(cwd), explicit)? else {
		return Ok(None);
	};
	workyard_state::Reader::open(&state, root).map(Some)
}
