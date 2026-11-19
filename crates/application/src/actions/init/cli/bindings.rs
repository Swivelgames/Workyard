use trine::cli::Bindings;

pub(in super::super) fn bindings() -> Bindings {
	Bindings::new()
		.positional("directory")
		.native_path("directory")
		.option("tasks", "tasks")
		.native_path("tasks")
}
