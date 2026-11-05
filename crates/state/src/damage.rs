//! Diagnostics for damaged state, independent of the operation inspecting it.
use workyard_core::{Problem, ProblemCode};

pub(crate) fn damaged(message: impl Into<String>) -> Problem {
	Problem::new(ProblemCode::DamagedState, message).remedy(
		"Preserve state and Git worktrees; inspect the independent evidence before repairing metadata.",
	)
}
