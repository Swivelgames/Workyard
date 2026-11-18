//! Recover admitted tasks without replaying successful or uncertain Git effects.
mod action;
mod cli;
mod contract;
mod dispatch;
mod input;
mod interface;

use crate::{contracts::Progress, presentation::cli::exit_status};
use trine::{
	Action, RegistrationError,
	application::{CliBinding, Entry},
};

pub use action::recover;
pub use contract::RecoverOutcome;
pub use input::RecoverInput;

type RecoverEntry = Entry<interface::Input, interface::Outcome, interface::Outcome, Progress>;

pub(crate) fn build_entry() -> Result<RecoverEntry, RegistrationError> {
	let handler = Action::new(
		"recover",
		"Resume interrupted task or workspace operations",
		dispatch::recover,
	)?;
	let cli = CliBinding::new(cli::bindings());
	let rpc = crate::presentation::structured::progress_binding();

	Ok(Entry::new(handler)
		.cli(
			cli,
			|output| Ok(cli::DispatchView::new(output)),
			|result| exit_status::operation(result, interface::Outcome::operation),
		)
		.rpc(rpc))
}
