use super::Recovery;
use crate::{
	Observer, Phase, Problem, ProblemCode, RecoveryGit, RecoveryState, SourceResolution, Task,
};

impl<G: RecoveryGit, S: RecoveryState, O: Observer> Recovery<'_, G, S, O> {
	pub(super) async fn create_missing(
		&self,
		task: &mut Task,
		index: usize,
		repository: &crate::Repository,
	) -> Result<(), Problem> {
		let options = task.creation_options(index);
		let participant = &task.participants[index];
		if matches!(
			participant.phase,
			Phase::WorktreeCreated | Phase::Pulling | Phase::Completed
		) {
			return Err(Problem::new(
				ProblemCode::RepositoryChanged,
				"Previously created worktree is missing; recovery will not recreate authored work.",
			)
			.remedy("Restore access or repair Git worktree relationships, then recover again."));
		}
		if participant.destination != task.root.join(&task.name).join(&participant.name) {
			return Err(Problem::new(
				ProblemCode::DamagedState,
				"Unstarted destination disagrees with admitted task placement.",
			));
		}

		let fetch_needed = !options.no_fetch
			&& repository.remote.is_some()
			&& matches!(participant.phase, Phase::NotStarted | Phase::Fetching);
		if fetch_needed {
			self.phase(task, index, Phase::Fetching).await?;
			self.store
				.record_effect(self.guard, task, index, crate::EffectState::Pending)?;
			let result = self.runner.fetch(repository, self.control).await;
			if result.is_ok() {
				task.participants[index].phase = Phase::Creating;
			}
			self.store
				.record_effect(self.guard, task, index, crate::EffectState::Settled)?;
			result?;
		}

		self.phase(task, index, Phase::Creating).await?;
		self.store.verify()?;
		self.runner.revalidate(repository, self.control).await?;
		let participant = &task.participants[index];
		let plan = self
			.runner
			.plan(
				repository,
				&task.name,
				options.checkout,
				participant.from.as_deref(),
				SourceResolution::Cached,
				self.control,
			)
			.await?;
		self.store.validate_destination(&participant.destination)?;
		self.store
			.record_effect(self.guard, task, index, crate::EffectState::Pending)?;
		let result = self
			.runner
			.add(
				repository,
				&task.name,
				&participant.destination,
				&plan,
				self.control,
			)
			.await;
		crate::settle_add::settle_add(result, task, index, self.runner, self.store, self.guard)
			.await
	}
}
