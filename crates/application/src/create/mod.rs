//! Create/checkout inputs, outcomes and handler. Interface assembly stays internal.
mod action;
mod contract;
mod input;
mod view;

use crate::common::Progress;
use trine::{
	Action, RegistrationError,
	application::{CliBinding, Entry},
	cli::{Alias, Bindings},
};

pub use action::create;
pub use contract::{CreateOutcome, ParticipantOutcome};
pub use input::{CreateInput, RepositoryInput};
use view::CreateView;

type CreateEntry = Entry<CreateInput, CreateOutcome, CreateOutcome, Progress>;

pub(crate) fn build_entry() -> Result<CreateEntry, RegistrationError> {
	let handler = Action::new(
		"create",
		"Create a task across explicit Git Repositories",
		action::create,
	)?;

	let checkout_shorthand = Alias::new("checkout").set("checkout", true);

	let cli = CliBinding::new(cli_bindings()).alias(checkout_shorthand);

	let rpc = crate::structured::progress_binding();

	Ok(Entry::new(handler)
		.cli(
			cli,
			|output| Ok(CreateView { output }),
			|result| crate::completion::operation(result, |outcome| &outcome.operation),
		)
		.rpc(rpc))
}

fn cli_bindings() -> Bindings {
	let bindings = Bindings::new();

	// Target branch name
	let bindings = bindings.positional("task").complete_group(
		"task",
		"Branches",
		crate::argument_completion::checkout,
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
			crate::argument_completion::common_remote,
		)
		// Common source branch
		.option("from", "from")
		.complete_group(
			"from",
			"References",
			crate::argument_completion::common_from,
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
			crate::argument_completion::repo_remote,
		)
		.option("repositories[].from", "repo-from")
		.complete_group(
			"repositories[].from",
			"References",
			crate::argument_completion::repo_from,
		)
}
