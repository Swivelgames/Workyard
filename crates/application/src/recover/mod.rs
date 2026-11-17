//! Recover admitted tasks without replaying successful or uncertain Git effects.
mod action;
mod contract;
mod input;
mod view;

use crate::common::Progress;
use trine::{
	Action, RegistrationError,
	application::{CliBinding, Entry},
	cli::Bindings,
};

pub use action::recover;
pub use contract::RecoverOutcome;
pub use input::RecoverInput;
use view::RecoverView;

type RecoverEntry = Entry<RecoverInput, RecoverOutcome, RecoverOutcome, Progress>;

pub(crate) fn build_entry() -> Result<RecoverEntry, RegistrationError> {
	let handler = Action::new("recover", "Recover interrupted task work", action::recover)?;

	let cli = CliBinding::new(cli_bindings());

	let rpc = crate::structured::progress_binding();

	Ok(Entry::new(handler)
		.cli(
			cli,
			|output| {
				Ok(RecoverView {
					output,
					started: false,
				})
			},
			|result| crate::completion::operation(result, |outcome| &outcome.operation),
		)
		.rpc(rpc))
}

fn cli_bindings() -> Bindings {
	let bindings = Bindings::new();

	// Target task
	let bindings = bindings.positional("task").complete_group(
		"task",
		"Tasks",
		crate::task_completion::suggest,
	);

	// Side-effects
	let bindings = bindings
		.option("accept_pull", "accept-pull")
		.complete_group(
			"accept_pull",
			"Participants",
			crate::argument_completion::unsettled_pull,
		)
		.option("undo", "undo")
		.option("yes", "yes")
		.option("force", "force");

	// Global arguments
	bindings
		.option("tasks_root", "tasks-root")
		.native_path("tasks_root")
		.option("state_dir", "state-dir")
		.native_path("state_dir")
}
