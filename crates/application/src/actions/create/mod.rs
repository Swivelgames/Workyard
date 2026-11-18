//! Create/checkout inputs, outcomes and handler. Interface assembly stays internal.
mod action;
mod cli;
mod contract;
mod input;
mod prepare;

use crate::{contracts::Progress, presentation::cli::exit_status};
use trine::{
	Action, RegistrationError,
	application::{CliBinding, Entry},
	cli::Alias,
};

pub use crate::contracts::RepositoryInput;
pub use action::create;
pub use contract::{CreateOutcome, ParticipantOutcome};
pub use input::CreateInput;

type CreateEntry = Entry<CreateInput, CreateOutcome, CreateOutcome, Progress>;

pub(crate) fn build_entry() -> Result<CreateEntry, RegistrationError> {
	let handler = Action::new(
		"create",
		"Create a task with a new branch in each repository",
		action::create,
	)?;

	let checkout_shorthand = Alias::new("checkout")
		.description("Create a task, reusing existing target branches where available")
		.set("checkout", true);
	let cli = CliBinding::new(cli::bindings()).alias(checkout_shorthand);
	let rpc = crate::presentation::structured::progress_binding();

	Ok(Entry::new(handler)
		.cli(
			cli,
			|output| Ok(cli::CreateView { output }),
			|result| exit_status::operation(result, |outcome| &outcome.operation),
		)
		.rpc(rpc))
}
