//! Execute participants sequentially, publishing verified membership before progress.
use crate::{Control, EffectState, Git, Observer, Phase, Problem, Repository, State, Task};

pub(super) struct Creation<'a, G, S: State, O> {
	pub runner: &'a G,
	pub control: &'a Control,
	pub observer: &'a O,
	pub store: &'a S,
	pub guard: &'a S::Admission,
}

impl<G: Git, S: State, O: Observer> Creation<'_, G, S, O> {
	pub(super) async fn run(
		&self,
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
					.record_effect(self.guard, task, index, EffectState::Pending)?;
				let result = self.runner.fetch(repository, self.control).await;
				let mut settled = task.clone();
				if result.is_ok() {
					settled.participants[index].phase = Phase::Creating;
				}
				self.store
					.record_effect(self.guard, &settled, index, EffectState::Settled)?;
				result?;
			}

			self.prepare_worktree(task, index, repository).await?;
			self.phase(task, index, Phase::Creating).await?;
			// Presentation can await indefinitely. Refresh protected observations
			// after delivery, immediately before the worktree effect.
			let (destination, plan) = self.prepare_worktree(task, index, repository).await?;
			self.control.check()?;
			self.store
				.record_effect(self.guard, task, index, EffectState::Pending)?;
			let result = self
				.runner
				.add(repository, &task.name, &destination, &plan, self.control)
				.await;
			crate::settle_add::settle_add(result, task, index, self.runner, self.store, self.guard)
				.await?;
			self.store.save(self.guard, task)?;

			let participant = &task.participants[index];
			for event in [Phase::WorktreeCreated, Phase::Completed] {
				self.observer
					.phase(&task.name, &participant.name, event.as_str())
					.await?;
				self.control.check()?;
			}
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
		self.control.check()?;
		crate::publication::phase(task, index, phase, self.store, self.guard, self.observer)
			.await?;
		self.control.check()
	}
}
