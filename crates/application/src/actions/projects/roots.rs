use super::{
	contract::{RootChanges, RootsInput},
	view::RootsView,
};
use crate::{
	contracts::{ProblemOutcome, Progress},
	execution::blocking_action,
	presentation::cli::{exit_status, workspace_binding},
};
use trine::{
	Action, InvocationContext, NativePath, RegistrationError,
	application::{CliBinding, Entry},
	cli::Bindings,
	structured::StructuredBinding,
};
use workyard_core::{Control, Problem, ProblemCode};
use workyard_state::{RootEdit, Store};

type RootsEntry = Entry<RootsInput, RootChanges, ProblemOutcome, Progress>;

pub(super) fn track_entry() -> Result<RootsEntry, RegistrationError> {
	build_entry(RootEdit::Track)
}

pub(super) fn untrack_entry() -> Result<RootsEntry, RegistrationError> {
	build_entry(RootEdit::Untrack)
}

fn build_entry(edit: RootEdit) -> Result<RootsEntry, RegistrationError> {
	let (name, description) = match edit {
		RootEdit::Track => ("track", "Register project discovery roots"),
		RootEdit::Untrack => (
			"untrack",
			"Unregister discovery roots and keep existing tasks and repositories",
		),
	};
	let action = Action::new(name, description, move |input, context| {
		update(edit, input, context)
	})?;
	let mut bindings = workspace_binding(Bindings::new().positional("paths").native_path("paths"));
	if matches!(edit, RootEdit::Untrack) {
		bindings = bindings.complete_group(
			"paths",
			"Registered roots",
			crate::presentation::cli::completion::projects::roots,
		);
	}
	let cli = CliBinding::new(bindings);
	let rpc = StructuredBinding::serde();
	let mcp = StructuredBinding::serde();

	Ok(Entry::new(action)
		.namespace(["projects"])
		.cli(
			cli,
			move |output| Ok(RootsView { output, edit }),
			exit_status::result,
		)
		.rpc(rpc)
		.mcp(mcp))
}

async fn update(
	edit: RootEdit,
	input: RootsInput,
	context: InvocationContext<Progress>,
) -> Result<RootChanges, ProblemOutcome> {
	blocking_action(async move {
		if input.paths.is_empty() {
			return Err(Problem::new(
				ProblemCode::InvalidInput,
				"Supply at least one project root.",
			)
			.into());
		}
		let control = Control::new(move || context.cancellation().is_cancelled());
		control.check()?;
		let state = crate::workspace::required(input.workspace.as_ref().map(NativePath::as_path))?;
		let store = Store::open_existing(&state, None)?;
		let paths: Vec<_> = input
			.paths
			.into_iter()
			.map(NativePath::into_path_buf)
			.collect();
		let changed = store.edit_project_roots(edit, &paths, &control).await?;
		Ok(RootChanges {
			changed: changed
				.into_iter()
				.map(|p| NativePath::new(p).expect("root path"))
				.collect(),
		})
	})
	.await
}
