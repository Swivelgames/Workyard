use super::Recovery;
use crate::{Observer, Phase, Problem, ProblemCode, RecoveryGit, RecoveryState, Task};

impl<G: RecoveryGit, S: RecoveryState, O: Observer> Recovery<'_, G, S, O> {
	pub(super) async fn participant(
		&self,
		task: &mut Task,
		index: usize,
		accepted: &[String],
	) -> Result<(), Problem> {
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
				.resume_existing(task, index, accepted, &repository, destination, previous)
				.await;
		}

		self.create_missing(task, index, &repository).await?;
		self.complete_created(task, index, &repository).await
	}

	async fn resume_existing(
		&self,
		task: &mut Task,
		index: usize,
		accepted: &[String],
		repository: &crate::Repository,
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
				self.publication_attempted
					.store(true, std::sync::atomic::Ordering::Relaxed);
				self.store
					.record_effect(self.guard, task, index, crate::EffectState::Settled)?;
			}
			return Ok(());
		}

		if participant.phase == Phase::Pulling {
			if !accepted.contains(&participant.name) {
				let message = format!(
					"Pull outcome for {:?} needs explicit acceptance after manual repair.",
					participant.name
				);
				let remedy = format!(
					"Inspect git status and complete or abort the pull yourself. Then run recover {:?} --state-dir {:?} --accept-pull {:?}. No pull will be replayed.",
					task.name, task.state_dir, participant.name
				);
				return Err(Problem::new(ProblemCode::GitFailed, message).remedy(remedy));
			}
			return self.phase(task, index, Phase::Completed).await;
		}

		participant.phase = Phase::WorktreeCreated;
		self.complete_created(task, index, repository).await
	}

	async fn complete_created(
		&self,
		task: &mut Task,
		index: usize,
		repository: &crate::Repository,
	) -> Result<(), Problem> {
		if task.pull {
			self.pull(task, index, repository).await?;
		}
		self.phase(task, index, Phase::Completed).await
	}
}
