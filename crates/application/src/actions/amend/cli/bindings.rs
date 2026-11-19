use crate::presentation::cli::{completion, project_bindings};
use trine::cli::Bindings;

pub(in super::super) fn bindings() -> Bindings {
	let bindings = Bindings::new().positional("task").complete_group(
		"task",
		"Tasks",
		completion::tasks::suggest,
	);

	let bindings = bindings
		.option("remove", "remove")
		.complete_group("remove", "Participants", completion::task_members)
		.option("yes", "yes")
		.option("force", "force");

	project_bindings(bindings)
}
