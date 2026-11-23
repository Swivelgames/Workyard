use trine::cli::Bindings;

pub(in super::super) fn bindings() -> Bindings {
	Bindings::new()
		.positional("directory")
		.native_path("directory")
		.option("tasks", "tasks")
		.native_path("tasks")
		.option("detached_tasks", "detached-tasks")
		.native_path("detached_tasks")
		.option("tracked_only", "tracked-only")
		.option("track", "track")
		.native_path("track")
}
