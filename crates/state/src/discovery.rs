use std::path::{Path, PathBuf};
use workyard_core::{Problem, ProblemCode};

pub fn discover_state(start: &Path, explicit: Option<&Path>) -> Result<PathBuf, Problem> {
	if let Some(state) = explicit {
		return crate::canonical_future(state);
	}
	find_state(start, explicit)?.ok_or_else(|| Problem::new(ProblemCode::DamagedState, "No ancestor .workyard state was found.")
		.remedy("Supply --state-dir for the existing authoritative store, or run beneath its workspace. Recovery cannot initialize state."))
}

/// Locate state without conflating absence with damaged or inaccessible data.
pub fn find_state(start: &Path, explicit: Option<&Path>) -> Result<Option<PathBuf>, Problem> {
	if let Some(state) = explicit {
		let state = crate::canonical_future(state)?;
		return Ok(crate::exists(&state)?.then_some(state));
	}
	let start = crate::canonical_future(start)?;
	for ancestor in start.ancestors() {
		let candidate = ancestor.join(".workyard");
		if crate::exists(&candidate)? {
			return Ok(Some(candidate));
		}
	}
	Ok(None)
}
