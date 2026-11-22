mod execution;
mod preflight;

use execution::Creation;

use crate::{Control, CreateRequest, Git, Observer, OperationOutcome, Problem, State};
use preflight::{preflight, validate_plans};

/// Establish the task and its participant worktrees.
pub async fn create(
	request: CreateRequest,
	runner: &impl Git,
	control: &Control,
	context: &impl Observer,
	store: &impl State,
) -> Result<crate::CreateResult, Problem> {
	control.check()?;
	let guard = store.admit(&request.task)?;
	let (mut task, repositories) = preflight(&request, runner, control).await?;
	store.ensure_new(&guard, &request.task)?;
	task.generation = store.next_generation(&guard)?;

	let record_path = store.record_path(&guard);
	let destination = request.tasks_root.join(&request.task);
	store.validate_destination(&destination)?;
	for participant in &task.participants {
		store.validate_destination(&participant.destination)?;
	}
	validate_plans(&task, &repositories, runner, control).await?;

	control.check()?;
	store.begin(&guard, &task)?;

	let creation = Creation {
		runner,
		control,
		observer: context,
		store,
		guard: &guard,
	};
	let operation = creation.run(&mut task, &repositories).await;

	if let Err(mut problem) = operation {
		task.operation.outcome = if problem.code == crate::ProblemCode::Cancelled {
			OperationOutcome::Cancelled
		} else {
			OperationOutcome::Failed
		};
		problem.remedy.push_str(&format!(
			" Inspect {record_path:?} and Git worktree/status output for each listed path. \
			Successful work is preserved; do not rerun create on this task."
		));
		task.problem = Some(problem);
	} else {
		task.operation.outcome = OperationOutcome::Succeeded;
	}

	let mut invocation = task.operation;
	let mut invocation_problem = task.problem.clone();
	task.preserve_settled_cancellation();

	if let Err(problem) = store.save(&guard, &task) {
		task.operation.outcome = OperationOutcome::Failed;
		task.problem = Some(problem.clone());
		invocation.outcome = OperationOutcome::Failed;
		invocation_problem = Some(problem);
	}

	Ok(crate::CreateResult {
		operation: invocation,
		task,
		record: record_path,
		problem: invocation_problem,
	})
}
