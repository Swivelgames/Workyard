//! Create/checkout inputs, outcomes and handler. Interface assembly stays internal.
use std::num::NonZeroUsize;
use trine::{
	Action, RegistrationError,
	application::{CliBinding, Entry},
	cli::{Alias, Bindings},
	structured::StructuredBinding,
};

mod action;
mod contract;
mod input;
mod view;

use crate::common::Progress;

pub use action::create;
pub use contract::{CreateOutcome, ParticipantOutcome};
pub use input::{CreateInput, RepositoryInput};
use view::CreateView;

type CreateEntry = Entry<CreateInput, CreateOutcome, CreateOutcome, Progress>;

pub(crate) fn create_action() -> Result<CreateEntry, RegistrationError> {
	let handler = Action::new(
		"create",
		"Create a task across explicit Git Repositories",
		action::create,
	)?;

	let checkout_shorthand = Alias::new("checkout").set("checkout", true);

	let cli = CliBinding::new(gen_args()).alias(checkout_shorthand);

	Ok(Entry::new(handler)
		.cli(cli, |output| Ok(CreateView { output }))
		.rpc(StructuredBinding::serde_with_progress(
			|event: Progress| {
				serde_json::to_value(event).map_err(|error| {
					trine::error::Failure::new(trine::error::FailureKind::EncodingFailed, error)
				})
			},
			NonZeroUsize::new(16).expect("nonzero capacity"),
		)))
}

fn gen_args() -> Bindings {
	let bindings = Bindings::new();

	// Target branch name
	let bindings = bindings.positional("task");

	// Options
	let bindings = bindings
		.option("checkout", "checkout")
		.option("no_fetch", "no-fetch")
		.option("pull", "pull");

	// Global arguments
	let bindings = bindings
		.option("tasks_root", "tasks-root")
		.native_path("tasks_root")
		.option("state_dir", "state-dir")
		.native_path("state_dir");

	// Common Selections
	let bindings = bindings
		// Common git remote
		.option("remote", "remote")
		// Common source branch
		.option("from", "from");

	// Repo Selection
	bindings
		.group("repositories", "path", "repo")
		.native_path("repositories[].path")
		.option("repositories[].name", "repo-name")
		.option("repositories[].remote", "repo-remote")
		.option("repositories[].from", "repo-from")
}
