//! Retire membership only after every managed worktree has been removed safely.
mod consent;
mod restoration;
mod verification;

use crate::{
	Control, EffectState, Operation, OperationKind, OperationOutcome, Problem, ProblemCode,
	RecoveryState, Removal, RemovalDirection, RemovalGit, RemovalObserver, RemovalOptions,
	RemovalParticipant, RemovalPhase, RemovalResult, Task, TaskLifecycle,
};

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

	let record_available = record.recorded.is_some();
	let original = record
		.recorded
		.clone()
		.unwrap_or_else(|| record.task.clone());
	let needs_attention = !record_available
		|| original.problem.is_some()
		|| original.operation.outcome != OperationOutcome::Succeeded
		|| record
			.task
			.participants
			.iter()
			.any(|participant| participant.phase != crate::Phase::Completed)
		|| record.task.removal.as_ref().is_some_and(|removal| {
			removal.direction == RemovalDirection::Remove
				|| removal.participants.iter().any(|participant| {
					matches!(
						participant.phase,
						RemovalPhase::Removed | RemovalPhase::Restoring | RemovalPhase::Recreated
					)
				})
		});

	let mut task = record.task.clone();
	restoration::retain_recorded_identity(&record, &mut task);
	let path = store.record_path(&guard);
	if recovering
		&& !(options.undo && task.lifecycle == TaskLifecycle::Retired)
		&& verification::unchanged(&record, &mut task, git, control).await?
	{
		control.check()?;
		store.verify()?;
		let mut operation = original.operation.recovery();
		operation.outcome = OperationOutcome::Succeeded;
		return Ok(RemovalResult {
			nothing_to_recover: true,
			task: original,
			record: path,
			operation,
			problem: None,
			mutation_started: false,
			inspected: true,
			record_available,
			needs_attention: false,
		});
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
			record_available,
			needs_attention,
		});
	}

	if options.undo && task.removal.is_none() {
		return Err(Problem::new(
			ProblemCode::InvalidInput,
			"This task has no removal to undo.",
		));
	}

	if !recovering
		&& task.removal.as_ref().is_some_and(|removal| {
			removal.direction == RemovalDirection::Restore
				&& removal.participants.iter().all(|participant| {
					matches!(
						participant.phase,
						RemovalPhase::Present | RemovalPhase::Restored
					)
				})
		}) {
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
	task.operation.outcome = match &result {
		Ok(()) => OperationOutcome::Succeeded,
		Err(problem) if problem.code == ProblemCode::Cancelled => OperationOutcome::Cancelled,
		Err(_) => OperationOutcome::Failed,
	};
	task.problem = result.err();
	let inspected = run.inspected
		&& task.problem.as_ref().is_none_or(|problem| {
			matches!(
				problem.code,
				ProblemCode::Cancelled | ProblemCode::DecisionRequired
			)
		});

	let operation = task.operation;
	let problem = task.problem.clone();
	if !run.publication_attempted {
		return Ok(RemovalResult {
			nothing_to_recover: false,
			task: original,
			record: path,
			operation,
			problem,
			mutation_started: false,
			inspected,
			record_available,
			needs_attention,
		});
	}

	// Once publication was attempted, preserve uncertainty and partial effects.
	// Never replace pending writer evidence with a settled final-result snapshot.
	if let Err(problem) = store.save(&guard, &task) {
		task.operation.outcome = OperationOutcome::Failed;
		task.problem = Some(problem);
	}
	Ok(RemovalResult {
		nothing_to_recover: false,
		operation: task.operation,
		problem: task.problem.clone(),
		task,
		record: path,
		mutation_started: true,
		inspected,
		record_available: true,
		needs_attention,
	})
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

		let mut observations = Vec::new();
		for (index, participant) in task.participants.iter().enumerate() {
			self.control.check()?;
			if task
				.removal
				.as_ref()
				.is_some_and(|r| r.participants[index].phase == RemovalPhase::Removed)
			{
				observations.push(None);
				continue;
			}
			let observation = self.git.inspect_removal(participant, self.control).await?;
			self.observer
				.disclose(&participant.name, &observation)
				.await?;
			observations.push(Some(observation));
		}

		self.inspected = true;
		self.authorize_task(&task.name, &observations).await?;

		if task.removal.is_none() {
			task.removal = Some(Removal {
				direction: RemovalDirection::Remove,
				retired_at: None,
				participants: observations
					.iter()
					.map(|observation| {
						let observation = observation
							.as_ref()
							.expect("initial participant inspection");
						RemovalParticipant {
							branch: observation.branch.clone(),
							dirty_at_start: !observation.dirty_paths.is_empty(),
							phase: RemovalPhase::Present,
						}
					})
					.collect(),
			});
		}

		for (index, observation) in observations.into_iter().enumerate() {
			let Some(mut disclosed) = observation else {
				continue;
			};
			self.control.check()?;
			self.store.verify()?;
			loop {
				let current = self
					.git
					.inspect_removal(&task.participants[index], self.control)
					.await?;
				if !current.same_worktree(&disclosed) || current.branch != disclosed.branch {
					return Err(Problem::new(
						ProblemCode::RepositoryChanged,
						"Worktree or branch changed after disclosure; inspect and retry.",
					));
				}
				if current.dirty_token != disclosed.dirty_token && !current.dirty_paths.is_empty() {
					self.observer
						.disclose(&task.participants[index].name, &current)
						.await?;
					self.authorize_changed_files(&task.participants[index].name)
						.await?;
					disclosed = current;
					continue;
				}
				disclosed = current;
				break;
			}

			task.participants[index].destination =
				disclosed.destination.clone().expect("verified destination");
			task.removal.as_mut().expect("removal intent").participants[index].branch =
				disclosed.branch.clone();
			task.removal.as_mut().expect("removal intent").participants[index].dirty_at_start |=
				!disclosed.dirty_paths.is_empty();

			self.control.check()?;
			self.publication_attempted = true;
			self.store
				.record_effect(self.guard, task, index, EffectState::Pending)?;
			let result = self
				.git
				.remove_worktree(&disclosed, !disclosed.dirty_paths.is_empty(), self.control)
				.await;
			if result
				.as_ref()
				.is_err_and(|problem| problem.code == ProblemCode::SupervisionFailed)
			{
				return result;
			}
			if result.is_ok() {
				task.removal.as_mut().expect("removal intent").participants[index].phase =
					RemovalPhase::Removed;
			}
			self.publish(task, index)?;
			result?;
			self.observer
				.phase(&task.name, &task.participants[index].name, "removed")
				.await?;
		}

		task.lifecycle = TaskLifecycle::Retired;
		task.operation.outcome = OperationOutcome::Succeeded;
		task.removal.as_mut().expect("removal intent").retired_at = Some(
			std::time::SystemTime::now()
				.duration_since(std::time::UNIX_EPOCH)
				.map_err(|_| {
					Problem::new(ProblemCode::IoError, "System clock precedes Unix epoch.")
				})?
				.as_millis()
				.try_into()
				.map_err(|_| {
					Problem::new(ProblemCode::IoError, "Retirement timestamp overflow.")
				})?,
		);
		self.publish(task, 0)?;
		self.store.cleanup_retired(self.guard, task)
	}

	fn publish(&mut self, task: &Task, index: usize) -> Result<(), Problem> {
		self.publication_attempted = true;
		self.store
			.record_effect(self.guard, task, index, EffectState::Settled)?;
		self.store.save(self.guard, task)
	}
}
