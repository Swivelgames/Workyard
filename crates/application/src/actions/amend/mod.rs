//! Amend inputs, outcomes and interface assembly.
mod action;
mod cli;
mod contract;
mod input;
mod mcp;
mod prepare;

use crate::{
	contracts::Progress, execution::removal::RemovalRequest, presentation::cli::exit_status,
};
pub use action::amend;
pub use contract::AmendOutcome;
pub use input::AmendInput;
use trine::{
	Action, RegistrationError,
	application::{CliBinding, Entry},
};

type AmendEntry = Entry<RemovalRequest<AmendInput>, AmendOutcome, AmendOutcome, Progress>;

pub(crate) fn build_entry() -> Result<AmendEntry, RegistrationError> {
	let handler = Action::new(
		"amend",
		"Add, rename, reconnect or remove task participants",
		action::execute,
	)?;
	let cli = CliBinding::new(cli::bindings());
	let rpc = crate::presentation::structured::progress_binding();
	let mcp = crate::presentation::mcp::progress_binding();

	Entry::new(handler)
		.cli(
			cli,
			|output| {
				Ok(cli::AmendView {
					output,
					progress_open: false,
				})
			},
			|result| exit_status::operation(result, |outcome| &outcome.operation),
		)
		.rpc(rpc)
		.mcp_with_input(
			mcp,
			"Add, rename or remove participants of one named task. Review any disclosed files before approving discard.",
			mcp::McpAmendInput::into,
		)
}
