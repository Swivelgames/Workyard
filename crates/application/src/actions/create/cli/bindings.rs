use crate::presentation::cli::{completion, project_bindings};
use trine::cli::Bindings;

const EXAMPLES: &str = concat!(
	"Examples\n",
	"    workyard create feat/payments api web\n",
	"    workyard checkout feat/payments api --repo web --repo-remote upstream\n",
	"\n",
	"    --remote and --from apply to all repositories.\n",
	"    --repo-name, --repo-remote and --repo-from apply to the preceding --repo.",
);

pub(in super::super) fn bindings() -> Bindings {
	let bindings = Bindings::new().examples(EXAMPLES);

	let bindings =
		bindings
			.positional("task")
			.complete_group("task", "Branches", completion::checkout);

	project_bindings(bindings)
}
