//! Read-only workspace context, separate from task inventory and status.
mod action;
mod cli;
mod contract;
mod input;

use crate::{contracts::Progress, presentation::cli::exit_status};
pub use action::info;
pub use contract::{DiscoverySource, InfoOutcome};
pub use input::InfoInput;
use trine::{
	Action, RegistrationError,
	application::{CliBinding, Entry},
	structured::StructuredBinding,
};

type InfoEntry = Entry<InfoInput, InfoOutcome, InfoOutcome, Progress>;

pub(crate) fn build_entry() -> Result<InfoEntry, RegistrationError> {
	let handler = Action::new(
		"info",
		"Show workspace paths and configuration",
		action::info,
	)?;
	let cli = CliBinding::new(cli::bindings());
	let rpc = StructuredBinding::serde();
	let mcp = StructuredBinding::serde();

	Ok(Entry::new(handler)
		.cli(
			cli,
			|output| Ok(cli::InfoView { output }),
			exit_status::result,
		)
		.rpc(rpc)
		.mcp(mcp)
		.mcp_observation())
}
