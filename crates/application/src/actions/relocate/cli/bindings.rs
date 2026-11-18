use trine::cli::Bindings;

const EXAMPLES: &str = concat!(
	"Examples\n",
	"    workyard relocate ../new-workspace\n",
	"    workyard relocate ../new-workspace --keep-source\n",
	"    workyard relocate --new-state-dir ../state",
);

pub(in super::super) fn bindings() -> Bindings {
	let bindings = Bindings::new()
		.examples(EXAMPLES)
		.positional_alternative("new", "DESTINATION")
		.option("old", "old")
		.native_path("old")
		.option("old_tasks_root", "old-tasks-root")
		.native_path("old_tasks_root");

	let bindings = bindings
		.option("new", "new")
		.native_path("new")
		.option("new_tasks_root", "new-tasks-root")
		.native_path("new_tasks_root");

	let bindings = bindings
		.option("old_state_dir", "old-state-dir")
		.native_path("old_state_dir")
		.option("new_state_dir", "new-state-dir")
		.native_path("new_state_dir");

	bindings
		.option("move", "move")
		.option("keep_source", "keep-source")
}
