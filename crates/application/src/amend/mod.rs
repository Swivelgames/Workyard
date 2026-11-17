//! Amend inputs, outcomes and interface assembly.
mod action;
mod contract;
mod input;
mod view;

use crate::common::Progress;
pub use action::amend;
pub use contract::AmendOutcome;
pub use input::AmendInput;
use trine::{
	Action, RegistrationError,
	application::{CliBinding, Entry},
	cli::Bindings,
};

type AmendEntry = Entry<AmendInput, AmendOutcome, AmendOutcome, Progress>;

pub(crate) fn build_entry() -> Result<AmendEntry, RegistrationError> {
	let action = Action::new("amend", "Add, rename or reconnect task repositories", amend)?;
	let cli = CliBinding::new(cli_bindings());
	let rpc = crate::structured::progress_binding();
	Ok(Entry::new(action)
		.cli(
			cli,
			|output| Ok(view::AmendView { output }),
			|result| crate::completion::operation(result, |outcome| &outcome.operation),
		)
		.rpc(rpc))
}

fn cli_bindings() -> Bindings {
	let bindings = Bindings::new().positional("task").complete_group(
		"task",
		"Tasks",
		crate::task_completion::suggest,
	);

	let bindings = bindings
		.option("checkout", "checkout")
		.option("no_fetch", "no-fetch")
		.option("pull", "pull");

	let bindings = bindings
		.option("tasks_root", "tasks-root")
		.native_path("tasks_root")
		.option("state_dir", "state-dir")
		.native_path("state_dir");

	let bindings = bindings
		.option("remote", "remote")
		.complete_group(
			"remote",
			"Remotes",
			crate::argument_completion::common_remote,
		)
		.option("from", "from")
		.complete_group(
			"from",
			"References",
			crate::argument_completion::common_from,
		);

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
