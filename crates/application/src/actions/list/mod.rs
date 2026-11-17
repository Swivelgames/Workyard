//! Read-only list capability.
mod action;
mod cli;
mod contract;
mod input;

use crate::{contracts::Progress, presentation::cli::exit_status};
use trine::{
	Action, RegistrationError,
	application::{CliBinding, Entry},
	structured::StructuredBinding,
};

pub use action::list;
pub use contract::ListOutcome;
pub use input::ListInput;

type ListEntry = Entry<ListInput, ListOutcome, ListOutcome, Progress>;

pub(crate) fn build_entry() -> Result<ListEntry, RegistrationError> {
	let handler = Action::new("list", "Inspect Workyard tasks", action::list)?;
	let cli = CliBinding::new(cli::bindings());
	let rpc = StructuredBinding::serde();

	Ok(Entry::new(handler)
		.cli(
			cli,
			|output| Ok(cli::ListView { output }),
			exit_status::result,
		)
		.rpc(rpc))
}
