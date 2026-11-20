use super::Recovery;
use crate::{Observer, Phase, Problem, ProblemCode, RecoveryGit, RecoveryState, Task};

impl<G: RecoveryGit, S: RecoveryState, O: Observer> Recovery<'_, G, S, O> {
	pub(super) async fn participant(&self, task: &mut Task, index: usize) -> Result<(), Problem> {
		self.repair_location(task, index).await?;
		let observed = self
			.runner
			.reconcile(&task.participants[index], &task.name, self.control)
			.await?;
		if observed.operation_in_progress {
			let name = &task.participants[index].name;
			let message = format!(
				"Participant {name:?} has native Git locks or an unfinished Git operation."
			);
			return Err(Problem::new(ProblemCode::ResourceBusy, message)
				.remedy("Inspect git status, finish or abort the Git operation manually, and wait for native writers to stop. Workyard never removes Git locks."));
		}

		let repository = observed.repository;
		let previous = task.participants[index].clone();
		let participant = &mut task.participants[index];
		participant.source = repository.source.clone();
		participant.common_dir = repository.common_dir.clone();

		if let Some(destination) = observed.destination {
			return self
				.resume_existing(task, index, destination, previous)
				.await;
		}

		self.create_missing(task, index, &repository).await?;
		self.phase(task, index, Phase::Completed).await
	}

	async fn repair_location(&self, task: &mut Task, index: usize) -> Result<(), Problem> {
		let Some(relationship) = self
			.runner
			.recovery_relationship(&task.participants[index], self.control)
			.await?
		else {
			return Ok(());
		};
		if !relationship.needs_repair {
			return Ok(());
		}
		let participant = relationship.participant;
		let repository = crate::Repository {
			source: participant.source.clone(),
			common_dir: participant.common_dir.clone(),
			remote: participant.remote.clone(),
			identity: participant.backing_identity.clone(),
		};

		self.observer
			.phase(&task.name, &participant.name, "repairing_worktree_link")
			.await?;
		self.control.check()?;
		self.store.verify()?;
		self.publication_attempted
			.store(true, std::sync::atomic::Ordering::Relaxed);
		task.participants[index] = participant;
		self.store
			.record_effect(self.guard, task, index, crate::EffectState::Pending)?;
		let result = self
			.runner
			.repair_relationship(&repository, &task.participants[index], self.control)
			.await;
		if result
			.as_ref()
			.is_err_and(|problem| problem.code == ProblemCode::SupervisionFailed)
		{
			return result;
		}
		if result.is_ok() {
			task.participants[index].placement = None;
		}
		self.store
			.record_effect(self.guard, task, index, crate::EffectState::Settled)?;
		result
	}

	async fn resume_existing(
		&self,
		task: &mut Task,
		index: usize,
		destination: std::path::PathBuf,
		previous: crate::Participant,
	) -> Result<(), Problem> {
		let participant = &mut task.participants[index];
		participant.destination = destination;
		if participant.phase == Phase::Completed {
			let cache_changed = participant.source != previous.source
				|| participant.common_dir != previous.common_dir
				|| participant.destination != previous.destination;
			if cache_changed {
				participant.placement = None;
				self.observer
					.phase(&task.name, &participant.name, "updating_worktree_location")
					.await?;
				self.control.check()?;
				self.store.verify()?;
				self.publication_attempted
					.store(true, std::sync::atomic::Ordering::Relaxed);
				self.store
					.record_effect(self.guard, task, index, crate::EffectState::Settled)?;
			}
			return Ok(());
		}

		self.phase(task, index, Phase::Completed).await
	}
}
