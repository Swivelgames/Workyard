use super::{
	contract::{Inventory, ListInput, Project, Root},
	view::ListView,
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
	Ok(Entry::new(action)
		.namespace(["projects"])
		.cli(cli, |output| Ok(ListView(output)), exit_status::result)
		.rpc(rpc))
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
		let roots = discovery
			.coverage
			.into_iter()
			.map(|root| {
				let status = if root.missing {
					"missing"
				} else if root.issues.is_empty() {
					"available"
				} else {
					"incomplete"
				};
				Root {
					path: NativePath::new(root.root).expect("root path"),
					status,
					issues: root.issues.into_iter().map(Into::into).collect(),
				}
			})
			.collect();
		let mut projects: Vec<_> = discovery
			.catalog
			.projects()
			.map(|project| Project {
				reference: NativePath::new(&project.reference).expect("project reference"),
				path: NativePath::new(&project.path).expect("project path"),
			})
			.collect();
		projects.sort_by(|a, b| a.reference.as_path().cmp(b.reference.as_path()));
		Ok(Inventory {
			roots,
			projects,
			guidance: None,
		})
	})
	.await
}
