//! Shared project selection, overrides and Git options for create and amend.
use super::{completion, workspace_binding};
use trine::cli::Bindings;

pub(crate) fn project_bindings(bindings: Bindings) -> Bindings {
	let bindings = bindings
		.option("checkout", "checkout")
		.option("no_fetch", "no-fetch");

	let bindings = workspace_binding(bindings)
		.positional("projects")
		.native_path("projects")
		.complete_group("projects", "Projects", completion::projects::suggest);

	let bindings = bindings
		.option("remote", "remote")
		.complete_group("remote", "Remotes", completion::common_remote)
		.option("from", "from")
		.complete_group("from", "References", completion::common_from);

	bindings
		.group("repositories", "path", "repo")
		.native_path("repositories[].path")
		.complete_group(
			"repositories[].path",
			"Projects",
			completion::projects::repo,
		)
		.option("repositories[].name", "repo-name")
		.option("repositories[].remote", "repo-remote")
		.complete_group("repositories[].remote", "Remotes", completion::repo_remote)
		.option("repositories[].from", "repo-from")
		.complete_group("repositories[].from", "References", completion::repo_from)
}
