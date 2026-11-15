//! Remove admitted tasks without replaying successful or uncertain Git effects.
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

pub use action::remove;
pub use contract::RemoveOutcome;
pub use input::RemoveInput;
use view::RemoveView;

type RemoveEntry = Entry<RemoveInput, RemoveOutcome, RemoveOutcome, Progress>;

pub(crate) fn build_entry() -> Result<RemoveEntry, RegistrationError> {
	let handler = Action::new(
		"remove",
		"Remove task worktrees and retire membership",
		action::remove,
	)?;

	let cli = CliBinding::new(cli_bindings());

	let rpc = crate::structured::progress_binding();

	Ok(Entry::new(handler)
		.cli(cli, |output| {
			Ok(RemoveView {
				output,
				started: false,
			})
		})
		.rpc(rpc))
}

fn cli_bindings() -> Bindings {
	let bindings = Bindings::new();

	// Target task
	let bindings = bindings.positional("task");

	// Side-effects
	let bindings = bindings.option("yes", "yes").option("force", "force");

	// Global arguments
	bindings
		.option("tasks_root", "tasks-root")
		.native_path("tasks_root")
		.option("state_dir", "state-dir")
		.native_path("state_dir")
}
