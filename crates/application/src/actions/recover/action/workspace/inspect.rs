//! Resolve ordinary Git locations first. Scan the tracked roots once when a clone moved.
use workyard_core::{Control, Participant, Problem, ProblemCode, projects::Catalog};
use workyard_git::{Placement, Runner, projects::Discovery};
use workyard_state::Reader;

pub(super) async fn participant(
	participant: &Participant,
	reader: &Reader,
	runner: &Runner,
	control: &Control,
	projects: &mut Option<Catalog>,
) -> Result<Placement, Problem> {
	if participant.worktree_identity.is_none() {
		return Err(Problem::new(
			ProblemCode::RepositoryChanged,
			"Participant has no verified worktree yet.",
		));
	}
	let error = match runner.inspect_placement(participant, None, control).await {
		Ok(result) => return Ok(result),
		Err(error) => error,
	};
	control.check()?;
	if projects.is_none() {
		*projects = Some(
			Discovery::scan(
				&reader.project_roots()?,
				reader.state(),
				reader.root(),
				runner,
				control,
			)
			.await?
			.catalog,
		);
	}
	let project = projects
		.as_ref()
		.expect("scanned")
		.projects()
		.find(|project| project.identity == participant.backing_identity);
	let Some(project) = project else {
		return Err(error);
	};
	runner
		.inspect_placement(participant, Some(&project.path), control)
		.await
}
