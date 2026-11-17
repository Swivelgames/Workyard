//! Read-only status capability.
mod action;
mod contract;
mod input;
mod view;

use crate::common::Progress;
use trine::{
	Action, RegistrationError,
	application::{CliBinding, Entry},
	cli::Bindings,
	structured::StructuredBinding,
};

pub use action::status;
pub use contract::StatusOutcome;
pub use input::StatusInput;
use view::StatusView;

type StatusEntry = Entry<StatusInput, StatusOutcome, StatusOutcome, Progress>;

pub(crate) fn build_entry() -> Result<StatusEntry, RegistrationError> {
	let handler = Action::new("status", "Inspect Workyard tasks", action::status)?;

	let cli = CliBinding::new(cli_bindings());

	let rpc = StructuredBinding::serde();

	Ok(Entry::new(handler)
		.cli(
			cli,
			|output| Ok(StatusView { output }),
			crate::completion::result,
		)
		.rpc(rpc))
}

fn cli_bindings() -> Bindings {
	let bindings = Bindings::new();
	let bindings = bindings.positional("task");

	bindings
		.option("tasks_root", "tasks-root")
		.native_path("tasks_root")
		.option("state_dir", "state-dir")
		.native_path("state_dir")
}
