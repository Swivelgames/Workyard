use std::path::PathBuf;
use workyard_core::{Control, Problem};
use workyard_state::{RootEdit, Store};

/// A registration failure reports its own retry without changing task health.
pub(crate) async fn register(
	store: &Store,
	paths: &[PathBuf],
	control: &Control,
) -> Option<Problem> {
	let problem = store
		.edit_project_roots(RootEdit::Track, paths, control)
		.await
		.err()?;
	let message = format!("Could not register admitted projects: {}", problem.message);
	Some(Problem::new(problem.code, message).remedy(
		"Membership is retained. Retry with workyard projects track and the supplied repository paths.",
	))
}
