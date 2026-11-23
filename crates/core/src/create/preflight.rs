mod participants;

use crate::{Control, CreateRequest, Git, Problem, Repository, Task, overlaps};

pub(crate) async fn preflight(
	request: &CreateRequest,
	runner: &impl Git,
	control: &Control,
) -> Result<(Task, Vec<Repository>), Problem> {
	runner
		.validate_branch(&request.tasks_root, &request.task, control)
		.await?;
	let task_path = request.tasks_root.join(&request.task);
	if overlaps(&task_path, &request.state_dir) {
		return Err(Problem::new(
			crate::ProblemCode::PathOverlap,
			"Task destination overlaps its state directory.",
		));
	}

	let (participants, repositories) =
		participants::plan(request, &task_path, runner, control).await?;
	let task = Task {
		generation: 0,
		removal: None,
		name: request.task.clone(),
		root: request.tasks_root.clone(),
		state_dir: request.state_dir.clone(),
		participants,
		checkout: request.checkout,
		no_fetch: request.no_fetch,
		lifecycle: crate::TaskLifecycle::Active,
		operation: crate::Operation::create(),
		problem: None,
	};
	Ok((task, repositories))
}

pub(crate) async fn validate_plans(
	task: &Task,
	repositories: &[Repository],
	runner: &impl Git,
	control: &Control,
) -> Result<(), Problem> {
	let resolution = if task.no_fetch {
		crate::SourceResolution::Cached
	} else {
		crate::SourceResolution::FetchPending
	};
	let mut branch_conflicts = Vec::new();
	let mut conflicting_repositories = Vec::new();

	for (repository, participant) in repositories.iter().zip(&task.participants) {
		let plan = runner
			.plan(
				repository,
				&task.name,
				task.checkout,
				participant.from.as_deref(),
				resolution,
				control,
			)
			.await;
		match plan {
			Ok(_) => {}
			Err(problem) if problem.code == crate::ProblemCode::BranchExists => {
				branch_conflicts.push(problem.message);
				conflicting_repositories.push(repository.source.clone());
			}
			Err(problem) => return Err(problem),
		}
	}

	if !branch_conflicts.is_empty() {
		let mut problem = Problem::new(
			crate::ProblemCode::BranchExists,
			branch_conflicts.join("\n"),
		)
		.remedy("Use checkout or create --checkout to permit reuse of existing branches.");
		problem.repositories = conflicting_repositories;
		return Err(problem);
	}

	Ok(())
}
