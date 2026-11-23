use super::{
	cli::ListView,
	contract::{Inventory, ListInput},
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
use workyard_core::{Control, Problem};

type ListEntry = Entry<ListInput, Inventory, ProblemOutcome, Progress>;

pub(super) fn build_entry() -> Result<ListEntry, RegistrationError> {
	let action = Action::new(
		"list",
		"List available projects and their discovery roots",
		list,
	)?;
	let cli = CliBinding::new(workspace_binding(Bindings::new()));
	let rpc = StructuredBinding::serde();
	let mcp = StructuredBinding::serde();

	Ok(Entry::new(action)
		.namespace(["projects"])
		.cli(cli, |output| Ok(ListView { output }), exit_status::result)
		.rpc(rpc)
		.mcp(mcp)
		.mcp_observation())
}

async fn list(
	input: ListInput,
	context: InvocationContext<Progress>,
) -> Result<Inventory, ProblemOutcome> {
	blocking_action(async move {
		let control = Control::new(move || context.cancellation().is_cancelled());
		let workspace = input.workspace.as_ref().map(NativePath::as_path);
		let cwd = std::env::current_dir()
			.map_err(|error| Problem::io("Read current directory", error))?;
		let Some(reader) = crate::workspace::reader(&cwd, workspace)? else {
			return Ok(Inventory {
				guidance: Some(crate::guidance::workspace::missing_state(workspace)),
				..Inventory::default()
			});
		};
		let discovery = crate::projects::discover(&reader, &control).await?;
		let mut inventory = Inventory::from(discovery);
		inventory.explicit_workspace = input.workspace;
		inventory.workspace = reader
			.state()
			.parent()
			.and_then(|path| NativePath::new(path).ok());

		Ok(inventory)
	})
	.await
}
