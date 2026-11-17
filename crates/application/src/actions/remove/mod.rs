//! Remove admitted tasks without replaying successful or uncertain Git effects.
mod action;
mod cli;
mod contract;
mod input;

use crate::{contracts::Progress, presentation::cli::exit_status};
use trine::{
	Action, RegistrationError,
	application::{CliBinding, Entry},
};

pub use action::remove;
pub use contract::RemoveOutcome;
pub use input::RemoveInput;

type RemoveEntry = Entry<RemoveInput, RemoveOutcome, RemoveOutcome, Progress>;

pub(crate) fn build_entry() -> Result<RemoveEntry, RegistrationError> {
	let handler = Action::new(
		"remove",
		"Remove task worktrees and retire membership",
		action::remove,
	)?;
	let cli = CliBinding::new(cli::bindings());
	let rpc = crate::presentation::structured::progress_binding();

	Ok(Entry::new(handler)
		.cli(
			cli,
			|output| {
				Ok(cli::RemoveView {
					output,
					started: false,
				})
			},
			|result| exit_status::operation(result, |outcome| &outcome.operation),
		)
		.rpc(rpc))
}
