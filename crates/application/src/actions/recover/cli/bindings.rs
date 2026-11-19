use crate::presentation::cli::workspace_binding;
use trine::cli::Bindings;

const EXAMPLES: &str = concat!(
	"Examples\n",
	"    workyard recover    Repair worktree locations across the workspace\n",
	"    workyard recover feat/payments    Resume a task operation\n",
	"    workyard recover feat/payments --undo    Undo an interrupted removal",
);

pub(in super::super) fn bindings() -> Bindings {
	let bindings = Bindings::new().examples(EXAMPLES);

	// Target task
	let bindings = bindings.positional("task").complete_group(
		"task",
		"Tasks",
		crate::presentation::cli::completion::tasks::suggest,
	);

	// Side-effects
	let bindings = bindings
		.option("accept_pull", "accept-pull")
		.complete_group(
			"accept_pull",
			"Participants",
			crate::presentation::cli::completion::unsettled_pull,
		)
		.option("activate_here", "activate-here")
		.option("undo", "undo")
		.option("yes", "yes")
		.option("force", "force");

	// Global arguments
	workspace_binding(bindings)
}
