//! Read-only list capability.
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

pub use action::list;
pub use contract::ListOutcome;
pub use input::ListInput;
use view::ListView;

type ListEntry = Entry<ListInput, ListOutcome, ListOutcome, Progress>;

pub(crate) fn build_entry() -> Result<ListEntry, RegistrationError> {
	let handler = Action::new("list", "Inspect Workyard tasks", action::list)?;

	let cli = CliBinding::new(cli_bindings());

	let rpc = StructuredBinding::serde();

	Ok(Entry::new(handler)
		.cli(
			cli,
			|output| Ok(ListView { output }),
			crate::completion::result,
		)
		.rpc(rpc))
}

fn cli_bindings() -> Bindings {
	let bindings = Bindings::new();

	bindings
		.option("tasks_root", "tasks-root")
		.native_path("tasks_root")
		.option("state_dir", "state-dir")
		.native_path("state_dir")
}
