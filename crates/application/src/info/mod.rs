//! Read-only workspace context, separate from task inventory and status.
mod action;
mod contract;
mod input;
mod view;

use crate::common::Progress;
pub use action::info;
pub use contract::{DiscoverySource, InfoOutcome};
pub use input::InfoInput;
use trine::{
	Action, RegistrationError,
	application::{CliBinding, Entry},
	cli::Bindings,
	structured::StructuredBinding,
};
use view::InfoView;

type InfoEntry = Entry<InfoInput, InfoOutcome, InfoOutcome, Progress>;

pub(crate) fn build_entry() -> Result<InfoEntry, RegistrationError> {
	let handler = Action::new("info", "Inspect Workyard workspace context", action::info)?;
	let cli = CliBinding::new(cli_bindings());
	let rpc = StructuredBinding::serde();
	Ok(Entry::new(handler)
		.cli(
			cli,
			|output| Ok(InfoView { output }),
			crate::completion::result,
		)
		.rpc(rpc))
}

fn cli_bindings() -> Bindings {
	Bindings::new()
		.option("tasks_root", "tasks-root")
		.native_path("tasks_root")
		.option("state_dir", "state-dir")
		.native_path("state_dir")
}
