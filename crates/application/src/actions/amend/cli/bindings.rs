use crate::presentation::cli::workspace_binding;
use trine::cli::Bindings;

pub(in super::super) fn bindings() -> Bindings {
	let bindings = Bindings::new().positional("task").complete_group(
		"task",
		"Tasks",
		crate::presentation::cli::completion::tasks::suggest,
	);

	let bindings = bindings
		.option("checkout", "checkout")
		.option("no_fetch", "no-fetch")
		.option("pull", "pull");

	let bindings = workspace_binding(bindings);

	let bindings = bindings
		.option("remote", "remote")
		.complete_group(
			"remote",
			"Remotes",
			crate::presentation::cli::completion::common_remote,
		)
		.option("from", "from")
		.complete_group(
			"from",
			"References",
			crate::presentation::cli::completion::common_from,
		);

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
