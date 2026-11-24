use crate::{canonical_future, exists, workspace::checked_marker};
use std::path::{Path, PathBuf};
use workyard_core::{Problem, ProblemCode};

/// Locate the workspace marker for the requested placement.
pub fn discover_state(start: &Path, workspace: Option<&Path>) -> Result<PathBuf, Problem> {
	find_state(start, workspace)?.ok_or_else(|| {
		Problem::new(ProblemCode::WorkspaceNotFound, "No workspace found.")
			.remedy("Run workyard init in the workspace directory to get started.")
	})
}

/// An explicit workspace selects its own marker. Otherwise the nearest ancestor wins.
pub fn find_state(start: &Path, workspace: Option<&Path>) -> Result<Option<PathBuf>, Problem> {
	if let Some(workspace) = workspace {
		return marker_at(&canonical_future(workspace)?);
	}
	for ancestor in canonical_future(start)?.ancestors() {
		if let Some(marker) = marker_at(ancestor)? {
			return Ok(Some(marker));
		}
	}
	Ok(None)
}

fn marker_at(workspace: &Path) -> Result<Option<PathBuf>, Problem> {
	let marker = workspace.join(".workyard");
	if !exists(&marker)? {
		return Ok(None);
	}
	checked_marker(&marker).map(Some)
}
