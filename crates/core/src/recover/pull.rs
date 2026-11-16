use super::Recovery;
use crate::{Observer, Phase, Problem, ProblemCode, RecoveryGit, RecoveryState, Task};

impl<G: RecoveryGit, S: RecoveryState, O: Observer> Recovery<'_, G, S, O> {
	pub(super) async fn pull(
		&self,
		task: &mut Task,
		index: usize,
		repository: &crate::Repository,
	) -> Result<(), Problem> {
		self.phase(task, index, Phase::Pulling).await?;
		self.runner.revalidate(repository, self.control).await?;
		let participant = &task.participants[index];
		let current = self
			.runner
			.reconcile(participant, &task.name, self.control)
			.await?;
		if current.operation_in_progress
			|| current.destination.as_ref() != Some(&participant.destination)
		{
			return Err(Problem::new(
				ProblemCode::RepositoryChanged,
				"Participant changed while recovery progress was delivered.",
			)
			.remedy(
				"Inspect the participant Git operation and worktree location, then retry recovery.",
			));
		}
		self.store
			.record_effect(self.guard, task, index, crate::EffectState::Pending)?;
		let result = self
			.runner
			.pull(repository, &participant.destination, self.control)
			.await;
		if let Some(failure) =
			crate::optional_pull::settle_pull(result, task, index, self.store, self.guard)?
		{
			self.pull_failures
				.lock()
				.expect("pull failures")
				.push(failure);
		}
		Ok(())
	}
}
