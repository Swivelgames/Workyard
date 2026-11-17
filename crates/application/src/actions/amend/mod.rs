//! Amend inputs, outcomes and interface assembly.
mod action;
mod cli;
mod contract;
mod input;
mod prepare;

use crate::{contracts::Progress, presentation::cli::exit_status};
pub use action::amend;
pub use contract::AmendOutcome;
pub use input::AmendInput;
use trine::{
	Action, RegistrationError,
	application::{CliBinding, Entry},
};

type AmendEntry = Entry<AmendInput, AmendOutcome, AmendOutcome, Progress>;

pub(crate) fn build_entry() -> Result<AmendEntry, RegistrationError> {
	let handler = Action::new("amend", "Add, rename or reconnect task repositories", amend)?;
	let cli = CliBinding::new(cli::bindings());
	let rpc = crate::presentation::structured::progress_binding();

	Ok(Entry::new(handler)
		.cli(
			cli,
			|output| Ok(cli::AmendView { output }),
			|result| exit_status::operation(result, |outcome| &outcome.operation),
		)
		.rpc(rpc))
}
