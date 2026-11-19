//! Keep the outcome of this invocation separate from previously published task history.
use super::RemovalRun;
use crate::{
	OperationOutcome, Phase, Problem, ProblemCode, RecoveryRecord, RecoveryState, RemovalResult,
	Task,
};
use std::path::PathBuf;

pub(super) struct PriorState {
	task: Task,
	pub(super) record_available: bool,
	pub(super) needs_attention: bool,
}

impl PriorState {
	pub(super) fn from_record(record: &RecoveryRecord) -> Self {
		let record_available = record.recorded.is_some();
		let task = record.recorded.as_ref().unwrap_or(&record.task).clone();
		let needs_attention = !record_available
			|| task.problem.is_some()
			|| task.operation.outcome != OperationOutcome::Succeeded
			|| record
				.task
				.participants
				.iter()
				.any(|p| p.phase != Phase::Completed)
			|| record
				.task
				.removal
				.as_ref()
				.is_some_and(|removal| !removal.restoration_complete());

		Self {
			task,
			record_available,
			needs_attention,
		}
	}

	pub(super) fn nothing_to_recover(self, path: PathBuf) -> RemovalResult {
		let mut operation = self.task.operation.recovery();
		operation.outcome = OperationOutcome::Succeeded;

		RemovalResult {
			nothing_to_recover: true,
			task: self.task,
			record: path,
			operation,
			problem: None,
			mutation_started: false,
			inspected: true,
			record_available: self.record_available,
			needs_attention: false,
		}
	}
}

impl<G, S: RecoveryState, O> RemovalRun<'_, G, S, O> {
	pub(super) fn finish(
		&self,
		mut task: Task,
		result: Result<(), Problem>,
		prior: PriorState,
		path: PathBuf,
	) -> RemovalResult {
		task.operation.outcome = match &result {
			Ok(()) => OperationOutcome::Succeeded,
			Err(problem) if problem.code == ProblemCode::Cancelled => OperationOutcome::Cancelled,
			Err(_) => OperationOutcome::Failed,
		};
		task.problem = result.err();
		let inspected = self.inspected
			&& task.problem.as_ref().is_none_or(|problem| {
				matches!(
					problem.code,
					ProblemCode::Cancelled | ProblemCode::DecisionRequired
				)
			});

		if !self.publication_attempted {
			return RemovalResult {
				nothing_to_recover: false,
				task: prior.task,
				record: path,
				operation: task.operation,
				problem: task.problem,
				mutation_started: false,
				inspected,
				record_available: prior.record_available,
				needs_attention: prior.needs_attention,
			};
		}

		// Once publication was attempted, preserve uncertainty and partial effects.
		// Never replace pending writer evidence with a settled final-result snapshot.
		if let Err(problem) = self.store.save(self.guard, &task) {
			task.operation.outcome = OperationOutcome::Failed;
			task.problem = Some(problem);
		}
		RemovalResult {
			nothing_to_recover: false,
			operation: task.operation,
			problem: task.problem.clone(),
			task,
			record: path,
			mutation_started: true,
			inspected,
			record_available: true,
			needs_attention: prior.needs_attention,
		}
	}
}
