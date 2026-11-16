//! Sequential participant execution and invocation-only optional update failures.
use crate::{Control, Git, Observer, Phase, Problem, PullFailure, Repository, State, Task};

pub(super) struct Creation<'a, G, S: State, O> {
	pub runner: &'a G,
	pub control: &'a Control,
	pub observer: &'a O,
	pub store: &'a S,
	pub guard: &'a S::Admission,
	pub pull_failures: Vec<PullFailure>,
}

impl<G: Git, S: State, O: Observer> Creation<'_, G, S, O> {
	pub(super) async fn run(
		&mut self,
		task: &mut Task,
		repositories: &[Repository],
	) -> Result<(), Problem> {
		let task_path = task.root.join(&task.name);
		self.control.check()?;
		self.store.reserve(&task_path)?;

		for (index, repository) in repositories.iter().enumerate() {
			self.control.check()?;
			self.store.verify()?;
			self.runner.revalidate(repository, self.control).await?;
			if !task.no_fetch && repository.remote.is_some() {
				self.phase(task, index, Phase::Fetching).await?;
				self.runner.revalidate(repository, self.control).await?;
				self.store
					.record_effect(self.guard, task, index, crate::EffectState::Pending)?;
				let result = self.runner.fetch(repository, self.control).await;
				let mut settled = task.clone();
				if result.is_ok() {
					settled.participants[index].phase = Phase::Creating;
				}
				self.store.record_effect(
					self.guard,
					&settled,
					index,
					crate::EffectState::Settled,
				)?;
				result?;
			}

			self.prepare_worktree(task, index, repository).await?;
			self.phase(task, index, Phase::Creating).await?;
			// Presentation can await indefinitely. Refresh protected observations
			// after delivery, immediately before the worktree effect.
			let (destination, plan) = self.prepare_worktree(task, index, repository).await?;
			self.control.check()?;
			self.store
				.record_effect(self.guard, task, index, crate::EffectState::Pending)?;
			let result = self
				.runner
				.add(repository, &task.name, &destination, &plan, self.control)
				.await;
			crate::settle_add::settle_add(result, task, index, self.runner, self.store, self.guard)
				.await?;
			self.phase(task, index, Phase::WorktreeCreated).await?;

			if task.pull {
				self.phase(task, index, Phase::Pulling).await?;
				self.store
					.record_effect(self.guard, task, index, crate::EffectState::Pending)?;
				let result = self
					.runner
					.pull(repository, &destination, self.control)
					.await;
				if let Some(failure) =
					crate::optional_pull::settle_pull(result, task, index, self.store, self.guard)?
				{
					self.pull_failures.push(failure);
				}
			}

			self.phase(task, index, Phase::Completed).await?;
		}

		Ok(())
	}

	async fn prepare_worktree(
		&self,
		task: &Task,
		index: usize,
		repository: &Repository,
	) -> Result<(std::path::PathBuf, crate::BranchPlan), Problem> {
		self.store.verify()?;
		self.runner.revalidate(repository, self.control).await?;
		let participant = &task.participants[index];
		let plan = self
			.runner
			.plan(
				repository,
				&task.name,
				task.checkout,
				participant.from.as_deref(),
				crate::SourceResolution::Cached,
				self.control,
			)
			.await?;
		self.store.validate_destination(&participant.destination)?;
		Ok((participant.destination.clone(), plan))
	}

	async fn phase(&self, task: &mut Task, index: usize, phase: Phase) -> Result<(), Problem> {
		// Observed successes must be recorded even when cancellation raced completion.
		let observed = matches!(phase, Phase::WorktreeCreated | Phase::Completed);
		if !observed {
			self.control.check()?;
		}
		crate::publication::phase(task, index, phase, self.store, self.guard, self.observer)
			.await?;
		self.control.check()
	}
}
