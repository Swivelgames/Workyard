//! Initialize an explicitly selected workspace or the current directory.
mod action;
mod cli;
mod contract;
mod input;
mod skill;

use crate::{
	contracts::{ProblemOutcome, Progress},
	presentation::cli::exit_status,
};
pub use action::init;
pub use contract::InitOutcome;
pub use input::InitInput;
use trine::{
	Action, RegistrationError,
	application::{CliBinding, Entry},
	structured::StructuredBinding,
};

type InitEntry = Entry<InitInput, InitOutcome, ProblemOutcome, Progress>;

pub(crate) fn build_entry() -> Result<InitEntry, RegistrationError> {
	let handler = Action::new("init", "Initialize a Workyard workspace", action::init)?;
	let cli = CliBinding::new(cli::bindings());
	let rpc = StructuredBinding::serde();
	let mcp = StructuredBinding::serde();

	Ok(Entry::new(handler)
		.cli(
			cli,
			|output| Ok(cli::InitView { output }),
			exit_status::result,
		)
		.rpc(rpc)
		.mcp(mcp))
}
