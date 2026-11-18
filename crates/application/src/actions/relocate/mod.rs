//! Workspace movement input, execution and interface assembly.
mod action;
mod cli;
mod contract;
mod input;

use crate::{contracts::Progress, presentation::cli::exit_status};
use trine::{
	Action, RegistrationError,
	application::{CliBinding, Entry},
};

pub(crate) use action::recover_workspace;
pub use action::relocate;
pub(crate) use cli::RelocateView;
pub use contract::RelocateOutcome;
pub use input::RelocateInput;

type RelocateEntry = Entry<RelocateInput, RelocateOutcome, RelocateOutcome, Progress>;

pub(crate) fn build_entry() -> Result<RelocateEntry, RegistrationError> {
	let handler = Action::new(
		"relocate",
		"Move a workspace or its authoritative state",
		relocate,
	)?;
	let cli = CliBinding::new(cli::bindings());
	let rpc = crate::presentation::structured::progress_binding();

	Ok(Entry::new(handler)
		.cli(
			cli,
			|output| Ok(RelocateView { output }),
			|result| exit_status::operation(result, |outcome| &outcome.operation),
		)
		.rpc(rpc))
}
