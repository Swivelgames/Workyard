use crate::presentation::cli::workspace_binding;
use trine::cli::Bindings;

pub(in super::super) fn bindings() -> Bindings {
	let bindings = Bindings::new();

	// Target task
	let bindings = bindings.positional("task").complete_group(
		"task",
		"Tasks",
		crate::presentation::cli::completion::tasks::suggest,
	);

	// Side-effects
	let bindings = bindings.option("yes", "yes").option("force", "force");

	// Global arguments
	workspace_binding(bindings)
}
