mod preflight;

use crate::{
	Control, CreateRequest, Git, Observer, Phase, Problem, Repository, State, Task, TaskStatus,
};
use preflight::{preflight, validate_plans};

pub async fn create(
	request: CreateRequest,
	runner: &impl Git,
	control: &Control,
	context: &impl Observer,
	store: &impl State,
) -> Result<(Task, std::path::PathBuf), Problem> {
	control.check()?;
	let guard = store.admit(&request.task)?;
	let (mut task, repositories) = preflight(&request, runner, control).await?;
	store.ensure_new(&guard, &request.task)?;

	let record_path = store.record_path(&guard);
	let destination = request.tasks_root.join(&request.task);
	store.validate_destination(&destination)?;
	validate_plans(&task, &repositories, runner, control).await?;

	store.begin(&guard, &task)?;

	let operation = run_participants(
		&mut task,
		&repositories,
		runner,
		control,
		context,
		store,
		&guard,
	)
	.await;

	if let Err(mut problem) = operation {
		task.status = if problem.code == crate::ProblemCode::Cancelled {
			TaskStatus::Cancelled
		} else {
			TaskStatus::Failed
		};
		problem.remedy.push_str(&format!(
			" Inspect {record_path:?} and Git worktree/status output for each listed path. \
			Successful work is preserved; do not rerun create on this task."
		));
		task.problem = Some(problem);
	} else {
		task.status = TaskStatus::Completed;
	}

	if let Err(problem) = store.save(&guard, &task) {
		task.status = TaskStatus::Failed;
		task.problem = Some(problem);
	}

	Ok((task, record_path))
}

async fn run_participants<S: State>(
	task: &mut Task,
	repositories: &[Repository],
	runner: &impl Git,
	control: &Control,
	context: &impl Observer,
	store: &S,
	guard: &S::Admission,
) -> Result<(), Problem> {
	let task_path = task.root.join(&task.name);
	control.check()?;
	store.reserve(&task_path)?;

	for (index, repository) in repositories.iter().enumerate() {
		control.check()?;
		store.verify()?;
		runner.revalidate(repository, control).await?;
		if !task.no_fetch && repository.remote.is_some() {
			phase(task, index, Phase::Fetching, store, guard, context, control).await?;
			runner.revalidate(repository, control).await?;
			runner.fetch(repository, control).await?;
		}

		prepare_worktree(task, index, repository, runner, control, store).await?;
		phase(task, index, Phase::Creating, store, guard, context, control).await?;
		// Presentation can await indefinitely. Refresh protected observations
		// after delivery, immediately before the worktree effect.
		let (destination, plan) =
			prepare_worktree(task, index, repository, runner, control, store).await?;
		control.check()?;
		runner
			.add(repository, &task.name, &destination, &plan, control)
			.await?;
		phase(
			task,
			index,
			Phase::WorktreeCreated,
			store,
			guard,
			context,
			control,
		)
		.await?;

		if task.pull {
			phase(task, index, Phase::Pulling, store, guard, context, control).await?;
			runner.pull(repository, &destination, control).await?;
		}

		phase(
			task,
			index,
			Phase::Completed,
			store,
			guard,
			context,
			control,
		)
		.await?;
	}

	Ok(())
}

async fn prepare_worktree(
	task: &Task,
	index: usize,
	repository: &Repository,
	runner: &impl Git,
	control: &Control,
	store: &impl State,
) -> Result<(std::path::PathBuf, crate::BranchPlan), Problem> {
	store.verify()?;
	runner.revalidate(repository, control).await?;
	let participant = &task.participants[index];
	let plan = runner
		.plan(
			repository,
			&task.name,
			task.checkout,
			participant.from.as_deref(),
			crate::SourceResolution::Cached,
			control,
		)
		.await?;
	store.validate_destination(&participant.destination)?;
	Ok((participant.destination.clone(), plan))
}

async fn phase<S: State>(
	task: &mut Task,
	index: usize,
	phase: Phase,
	store: &S,
	guard: &S::Admission,
	context: &impl Observer,
	control: &Control,
) -> Result<(), Problem> {
	// Observed successes must be recorded even when cancellation raced completion.
	let observed = matches!(phase, Phase::WorktreeCreated | Phase::Completed);
	if !observed {
		control.check()?;
	}
	task.participants[index].phase = phase;
	store.save(guard, task)?;
	context
		.phase(
			&task.name,
			&task.participants[index].name,
			task.participants[index].phase.as_str(),
		)
		.await?;
	control.check()
}
