mod creation;
mod participant;
mod scope;

pub use scope::RecoveryScope;

use crate::{
	Control, Observer, OperationOutcome, Phase, Problem, ProblemCode, RecoveryGit, RecoveryState,
	Task,
};
use std::sync::atomic::{AtomicBool, Ordering};

struct Recovery<'a, G, S: RecoveryState, O> {
	runner: &'a G,
	control: &'a Control,
	observer: &'a O,
	store: &'a S,
	guard: &'a S::Admission,
	publication_attempted: AtomicBool,
}

/// Reconcile the task with recorded evidence and current Git facts.
pub async fn recover(
	name: &str,
	runner: &impl RecoveryGit,
	control: &Control,
	observer: &impl Observer,
	store: &impl RecoveryState,
) -> Result<crate::RecoveryResult, Problem> {
	recover_with_scope(
		name,
		RecoveryScope::Reconcile,
		runner,
		control,
		observer,
		store,
	)
	.await
}

/// Run recovery with the caller's explicit scope.
pub async fn recover_with_scope(
	name: &str,
	scope: RecoveryScope,
	runner: &impl RecoveryGit,
	control: &Control,
	observer: &impl Observer,
	store: &impl RecoveryState,
) -> Result<crate::RecoveryResult, Problem> {
	crate::validate_task(name)?;
	control.check()?;
	let guard = store.admit(name)?;
	let record = store.load(&guard, name)?;
	scope.authorize(&record.task)?;
	let mut task = prepare_recovery(record.task.clone());

	if let Some(index) = record.pending
		&& task.participants[index].phase != Phase::Fetching
	{
		return Err(record.writer_problem(index));
	}

	let path = store.record_path(&guard);
	let recovery = Recovery {
		runner,
		control,
		observer,
		store,
		guard: &guard,
		publication_attempted: AtomicBool::new(false),
	};
	let result = recovery.resume(&mut task).await;

	if result.is_ok()
		&& !recovery.publication_attempted.load(Ordering::Relaxed)
		&& record.matches_published(&task)
	{
		control.check()?;
		store.verify()?;
		return Ok(crate::RecoveryResult {
			admitted_roots: Vec::new(),
			operation: record
				.recorded
				.as_ref()
				.expect("verified published task")
				.operation,
			problem: None,
			task: record.recorded.expect("verified published task"),
			record: path,
			nothing_to_recover: true,
		});
	}

	// An unsuccessful observation is an invocation result, not new task history.
	// No publication means no managed effect has been admitted by this attempt.
	if result.is_err() && !recovery.publication_attempted.load(Ordering::Relaxed) {
		let problem = result.expect_err("failed recovery");
		let mut operation = task.operation;
		operation.outcome = if problem.code == ProblemCode::Cancelled {
			OperationOutcome::Cancelled
		} else {
			OperationOutcome::Failed
		};
		return Ok(crate::RecoveryResult {
			admitted_roots: Vec::new(),
			operation,
			problem: Some(problem),
			task: record.recorded.unwrap_or(record.task),
			record: path,
			nothing_to_recover: false,
		});
	}

	task.operation.outcome = match &result {
		Ok(()) => OperationOutcome::Succeeded,
		Err(problem) if problem.code == ProblemCode::Cancelled => OperationOutcome::Cancelled,
		Err(_) => OperationOutcome::Failed,
	};
	task.problem = result.err();
	let mut invocation = task.operation;
	let mut invocation_problem = task.problem.clone();
	task.preserve_settled_cancellation();

	if let Err(problem) = store.save(&guard, &task) {
		task.operation.outcome = OperationOutcome::Failed;
		task.problem = Some(problem.clone());
		invocation.outcome = OperationOutcome::Failed;
		invocation_problem = Some(problem);
	}
	let admitted_roots = record.newly_admitted_roots(&task);
	Ok(crate::RecoveryResult {
		admitted_roots,
		operation: invocation,
		problem: invocation_problem,
		task,
		record: path,
		nothing_to_recover: false,
	})
}

impl<G: RecoveryGit, S: RecoveryState, O: Observer> Recovery<'_, G, S, O> {
	async fn resume(&self, task: &mut Task) -> Result<(), Problem> {
		for index in 0..task.participants.len() {
			self.control.check()?;
			self.store.verify()?;
			self.participant(task, index).await?;
		}
		Ok(())
	}

	async fn phase(&self, task: &mut Task, index: usize, phase: Phase) -> Result<(), Problem> {
		self.publication_attempted.store(true, Ordering::Relaxed);
		// Reconciled observations are published before cancellation is returned.
		crate::publication::phase(task, index, phase, self.store, self.guard, self.observer)
			.await?;
		self.control.check()
	}
}

/// Recovery starts a new attempt without changing the observed participant facts.
fn prepare_recovery(mut task: Task) -> Task {
	task.operation = task.operation.recovery();
	task.problem = None;
	task
}
