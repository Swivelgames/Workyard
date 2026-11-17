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
	bindings
		.option("tasks_root", "tasks-root")
		.native_path("tasks_root")
		.option("state_dir", "state-dir")
		.native_path("state_dir")
}
