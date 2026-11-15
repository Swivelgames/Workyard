mod creation;
mod participant;
mod pull;

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

pub async fn recover(
	name: &str,
	accept_pull: &[String],
	runner: &impl RecoveryGit,
	control: &Control,
	observer: &impl Observer,
	store: &impl RecoveryState,
) -> Result<crate::RecoveryResult, Problem> {
	crate::validate_task(name)?;
	control.check()?;
	let guard = store.admit(name)?;
	let record = store.load(&guard, name)?;
	let mut task = prepare_recovery(record.task.clone());

	for accepted in accept_pull {
		if !task
			.participants
			.iter()
			.any(|p| &p.name == accepted && p.phase == Phase::Pulling)
		{
			return Err(Problem::new(
				ProblemCode::InvalidInput,
				format!(
					"--accept-pull {accepted:?} must name a participant with an unsettled pull outcome."
				),
			));
		}
	}

	if let Some(index) = record.pending
		&& task.participants[index].phase != Phase::Fetching
	{
		let participant = &task.participants[index].name;
		let message = format!(
			"Participant {participant:?} has no durable proof its managed Git writers stopped."
		);
		return Err(Problem::new(ProblemCode::ResourceBusy, message)
				.remedy("Do not retry or accept the effect based on a free task lock. Inspect the interrupted Workyard process and Git process tree. This release has no force or writer-quarantine override: preserve state, continue manual Git repair as needed, and restore verified settled evidence from a backup only if available. --accept-pull cannot bypass this check."));
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
	let result = recovery.resume(&mut task, accept_pull).await;

	if result.is_ok()
		&& !recovery.publication_attempted.load(Ordering::Relaxed)
		&& record.matches_published(&task)
	{
		control.check()?;
		store.verify()?;
		return Ok(crate::RecoveryResult {
			task: record.recorded.expect("verified published task"),
			record: path,
			nothing_to_recover: true,
		});
	}

	task.operation.outcome = match &result {
		Ok(()) => OperationOutcome::Succeeded,
		Err(problem) if problem.code == ProblemCode::Cancelled => OperationOutcome::Cancelled,
		Err(_) => OperationOutcome::Failed,
	};
	task.problem = result.err();
	if let Err(problem) = store.save(&guard, &task) {
		task.operation.outcome = OperationOutcome::Failed;
		task.problem = Some(problem);
	}
	Ok(crate::RecoveryResult {
		task,
		record: path,
		nothing_to_recover: false,
	})
}

impl<G: RecoveryGit, S: RecoveryState, O: Observer> Recovery<'_, G, S, O> {
	async fn resume(&self, task: &mut Task, accepted: &[String]) -> Result<(), Problem> {
		for index in 0..task.participants.len() {
			self.control.check()?;
			self.store.verify()?;
			self.participant(task, index, accepted).await?;
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
