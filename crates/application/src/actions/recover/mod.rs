//! Recover admitted tasks without replaying successful or uncertain Git effects.
mod action;
mod cli;
mod contract;
mod input;

use crate::{contracts::Progress, presentation::cli::exit_status};
use trine::{
	Action, RegistrationError,
	application::{CliBinding, Entry},
};

pub use action::recover;
pub use contract::RecoverOutcome;
pub use input::RecoverInput;

type RecoverEntry = Entry<RecoverInput, RecoverOutcome, RecoverOutcome, Progress>;

pub(crate) fn build_entry() -> Result<RecoverEntry, RegistrationError> {
	let handler = Action::new(
		"recover",
		"Resume or undo an interrupted task operation",
		action::recover,
	)?;
	let cli = CliBinding::new(cli::bindings());
	let rpc = crate::presentation::structured::progress_binding();

	Ok(Entry::new(handler)
		.cli(
			cli,
			|output| {
				Ok(cli::RecoverView {
					output,
					started: false,
				})
			},
			|result| exit_status::operation(result, |outcome| &outcome.operation),
		)
		.rpc(rpc))
}
