//! Read-only status capability.
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

pub use action::status;
pub use contract::StatusOutcome;
pub use input::StatusInput;

use contract::PresentedStatus;

type StatusEntry = Entry<StatusInput, PresentedStatus, PresentedStatus, Progress>;

pub(crate) fn build_entry() -> Result<StatusEntry, RegistrationError> {
	let handler = Action::new("status", "Inspect Workyard tasks", action::present)?;
	let cli = CliBinding::new(cli::bindings());
	let rpc = StructuredBinding::serde();

	Ok(Entry::new(handler)
		.cli(
			cli,
			|output| Ok(cli::StatusView { output }),
			exit_status::result,
		)
		.rpc(rpc))
}
