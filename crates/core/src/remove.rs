//! Retire membership only after every managed worktree has been removed safely.
mod completion;
pub(crate) mod consent;
mod restoration;
mod retirement;
mod verification;

use crate::{
	Control, EffectState, Operation, OperationKind, OperationOutcome, Problem, ProblemCode,
	RecoveryState, Removal, RemovalDirection, RemovalGit, RemovalObserver, RemovalOptions,
	RemovalResult, Task, TaskLifecycle,
};

use completion::PriorState;

struct RemovalRun<'a, G, S: RecoveryState, O> {
	git: &'a G,
	store: &'a S,
	guard: &'a S::Admission,
	control: &'a Control,
	observer: &'a O,
	options: RemovalOptions,
	publication_attempted: bool,
	inspected: bool,
}

pub async fn remove(
	name: &str,
	options: RemovalOptions,
	git: &impl RemovalGit,
	control: &Control,
	observer: &impl RemovalObserver,
	store: &impl RecoveryState,
) -> Result<RemovalResult, Problem> {
	run(name, options, false, git, control, observer, store).await
}

pub async fn recover_removal(
	name: &str,
	options: RemovalOptions,
	git: &impl RemovalGit,
	control: &Control,
	observer: &impl RemovalObserver,
	store: &impl RecoveryState,
) -> Result<RemovalResult, Problem> {
	run(name, options, true, git, control, observer, store).await
}

async fn run(
	name: &str,
	options: RemovalOptions,
	recovering: bool,
	git: &impl RemovalGit,
	control: &Control,
	observer: &impl RemovalObserver,
	store: &impl RecoveryState,
) -> Result<RemovalResult, Problem> {
	crate::validate_task(name)?;
	control.check()?;
	let guard = store.admit(name)?;
	let record = store.load(&guard, name)?;
	if let Some(index) = record.pending {
		return Err(record.writer_problem(index));
	}

	let prior = PriorState::from_record(&record);
	let mut task = record.task.clone();
	restoration::retain_recorded_identity(&record, &mut task);
	let path = store.record_path(&guard);
	if recovering
		&& !(options.undo && task.lifecycle == TaskLifecycle::Retired)
		&& verification::unchanged(&record, &mut task, git, control).await?
	{
		control.check()?;
		store.verify()?;
		return Ok(prior.nothing_to_recover(path));
	}

	if task.lifecycle == TaskLifecycle::Retired {
		if options.undo {
			return Err(Problem::new(
				ProblemCode::InvalidInput,
				"This task is already retired; undo applies only to incomplete removal.",
			)
			.remedy("Use workyard checkout to create fresh membership on the retained branches."));
		}
		if recovering {
			task.operation = task.operation.recovery();
		}
		task.operation.outcome = OperationOutcome::Succeeded;
		if recovering {
			task.problem = None;
			store.save(&guard, &task)?;
		}
		return Ok(RemovalResult {
			nothing_to_recover: false,
			operation: task.operation,
			problem: None,
			task,
			record: path,
			mutation_started: recovering,
			inspected: false,
			record_available: prior.record_available,
			needs_attention: prior.needs_attention,
		});
	}

	if options.undo && task.removal.is_none() {
		return Err(Problem::new(
			ProblemCode::InvalidInput,
			"This task has no removal to undo.",
		));
	}

	if !recovering
		&& task
			.removal
			.as_ref()
			.is_some_and(Removal::restoration_complete)
	{
		task.removal = None;
	}

	task.operation = Operation {
		kind: Some(if recovering {
			OperationKind::Recover
		} else {
			OperationKind::Remove
		}),
		continues: recovering.then_some(OperationKind::Remove),
		outcome: OperationOutcome::Pending,
	};
	task.problem = None;

	let mut run = RemovalRun {
		git,
		store,
		guard: &guard,
		control,
		observer,
		options,
		publication_attempted: false,
		inspected: false,
	};
	let result = run.execute(&mut task).await;
	Ok(run.finish(task, result, prior, path))
}

impl<G: RemovalGit, S: RecoveryState, O: RemovalObserver> RemovalRun<'_, G, S, O> {
	async fn execute(&mut self, task: &mut Task) -> Result<(), Problem> {
		if self.options.undo
			|| task
				.removal
				.as_ref()
				.is_some_and(|r| r.direction == RemovalDirection::Restore)
		{
			return self.restore(task).await;
		}

		self.retire(task).await
	}

	fn publish(&mut self, task: &Task, index: usize) -> Result<(), Problem> {
		self.publication_attempted = true;
		self.store
			.record_effect(self.guard, task, index, EffectState::Settled)?;
		self.store.save(self.guard, task)
	}
}
