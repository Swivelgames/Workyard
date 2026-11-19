use crate::presentation::cli::workspace_binding;
use trine::cli::Bindings;

const EXAMPLES: &str = concat!(
	"Examples\n",
	"    workyard create --repo ../api --repo ../web feat/payments\n",
	"    workyard checkout --repo ../api feat/payments\n",
	"\n",
	"    --remote and --from apply to all repositories.\n",
	"    --repo-name, --repo-remote and --repo-from apply to the preceding --repo.",
);

pub(in super::super) fn bindings() -> Bindings {
	let bindings = Bindings::new().examples(EXAMPLES);

	// Target branch name
	let bindings = bindings.positional("task").complete_group(
		"task",
		"Branches",
		crate::presentation::cli::completion::checkout,
	);

	// Options
	let bindings = bindings
		.option("checkout", "checkout")
		.option("no_fetch", "no-fetch")
		.option("pull", "pull");

	// Global arguments
	let bindings = workspace_binding(bindings);

	// Common Selections
	let bindings = bindings
		// Common git remote
		.option("remote", "remote")
		.complete_group(
			"remote",
			"Remotes",
			crate::presentation::cli::completion::common_remote,
		)
		// Common source branch
		.option("from", "from")
		.complete_group(
			"from",
			"References",
			crate::presentation::cli::completion::common_from,
		);

	// Repo Selection
	bindings
		.group("repositories", "path", "repo")
		.native_path("repositories[].path")
		.option("repositories[].name", "repo-name")
		.option("repositories[].remote", "repo-remote")
		.complete_group(
			"repositories[].remote",
			"Remotes",
			crate::presentation::cli::completion::repo_remote,
		)
		.option("repositories[].from", "repo-from")
		.complete_group(
			"repositories[].from",
			"References",
			crate::presentation::cli::completion::repo_from,
		)
}
