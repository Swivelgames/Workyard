use trine::cli::Bindings;

pub(in super::super) fn bindings() -> Bindings {
	let bindings = Bindings::new();

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
	let bindings = bindings
		.option("tasks_root", "tasks-root")
		.native_path("tasks_root")
		.option("state_dir", "state-dir")
		.native_path("state_dir");

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
