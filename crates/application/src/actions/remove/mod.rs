//! Remove admitted tasks without replaying successful or uncertain Git effects.
mod action;
mod cli;
mod contract;
mod input;

use crate::{
	contracts::Progress, execution::removal::RemovalRequest, presentation::cli::exit_status,
};
use trine::{
	Action, RegistrationError,
	application::{CliBinding, Entry},
};

pub use action::remove;
pub use contract::RemoveOutcome;
pub use input::RemoveInput;

type RemoveEntry = Entry<RemovalRequest<RemoveInput>, RemoveOutcome, RemoveOutcome, Progress>;

pub(crate) fn build_entry() -> Result<RemoveEntry, RegistrationError> {
	let handler = Action::new(
		"remove",
		"Remove task worktrees and retire membership",
		action::execute,
	)?;
	let cli = CliBinding::new(cli::bindings());
	let rpc = crate::presentation::structured::progress_binding();
	let mcp = crate::presentation::mcp::progress_binding();

	Entry::new(handler)
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
		.rpc(rpc)
		.mcp_with_input(
			mcp,
			"Retire a task and remove its worktrees. Review any disclosed files before approving discard.",
			input::McpRemoveInput::into,
		)
}
