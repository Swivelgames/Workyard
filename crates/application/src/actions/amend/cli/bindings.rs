use crate::presentation::cli::{completion, project_bindings};
use trine::cli::Bindings;

pub(in super::super) fn bindings() -> Bindings {
	let bindings = Bindings::new().positional("task").complete_group(
		"task",
		"Tasks",
		completion::tasks::suggest,
	);

	project_bindings(bindings)
}
