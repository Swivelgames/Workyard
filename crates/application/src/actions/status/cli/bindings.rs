use crate::presentation::cli::workspace_binding;
use trine::cli::Bindings;

pub(in super::super) fn bindings() -> Bindings {
	let bindings = Bindings::new();
	let bindings = bindings.positional("task").complete_candidates(
		"task",
		Some("Tasks"),
		crate::presentation::cli::completion::tasks::suggest,
	);

	workspace_binding(bindings)
}
