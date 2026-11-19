//! Recover admitted tasks without replaying successful or uncertain Git effects.
mod action;
mod cli;
mod contract;
mod input;
mod workspace;

use crate::{contracts::Progress, presentation::cli::exit_status};
use trine::{
	Action, RegistrationError,
	application::{CliBinding, Entry},
};

pub use action::recover;
pub use contract::{RecoverOutcome, RecoveryOutcome};
pub use input::RecoverInput;

type RecoverEntry = Entry<RecoverInput, RecoveryOutcome, RecoveryOutcome, Progress>;

pub(crate) fn build_entry() -> Result<RecoverEntry, RegistrationError> {
	let handler = Action::new(
		"recover",
		"Repair workspace locations or resume an interrupted task operation",
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
			|result| match result {
				Ok(RecoveryOutcome::Task(outcome)) => u8::from(
					outcome.operation.outcome != workyard_core::OperationOutcome::Succeeded,
				),
				_ => exit_status::result(result),
			},
		)
		.rpc(rpc))
}
