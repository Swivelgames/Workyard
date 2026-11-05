use std::path::{Path, PathBuf};
use workyard_core::{Problem, ProblemCode};

pub fn discover_state(start: &Path, explicit: Option<&Path>) -> Result<PathBuf, Problem> {
	if let Some(state) = explicit {
		return crate::canonical_future(state);
	}
	let start = crate::canonical_future(start)?;
	for ancestor in start.ancestors() {
		let candidate = ancestor.join(".workyard");
		if crate::exists(&candidate)? {
			return Ok(candidate);
		}
	}
	Err(Problem::new(ProblemCode::DamagedState, "No ancestor .workyard state was found.")
		.remedy("Supply --state-dir for the existing authoritative store, or run beneath its workspace. Recovery cannot initialize state."))
}
