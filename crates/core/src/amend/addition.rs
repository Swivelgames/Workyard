use super::{Amendment, plan::Change};
use crate::{
	AmendGit, AmendState, AmendmentChangeKind as Kind, EffectState, Observer, Phase, Problem,
	ProblemCode, SourceResolution, Task,
};

impl<G: AmendGit, S: AmendState, O: Observer> Amendment<'_, G, S, O> {
	pub(super) async fn add(&mut self, task: &mut Task, change: Change) -> Result<(), Problem> {
		self.git
			.revalidate(&change.repository, self.control)
			.await?;
		let before = task.clone();
		self.begin(task)?;
		let index = match change.existing {
			Some(index) => {
				task.participants[index] = change.participant;
				index
			}
			None => {
				task.participants.push(change.participant);
				task.participants.len() - 1
			}
		};
		let result = self
			.create_participant(task, index, &change.repository)
			.await;
		let observed_worktree = task.participants[index].worktree_identity.is_some()
			|| task.participants[index].phase == Phase::WorktreeCreated;
		if result.is_ok() || observed_worktree || self.uncertain {
			return result;
		}

		if self
			.store
			.validate_destination(&task.participants[index].destination)
			.is_err()
		{
			self.changed = true;
			return result;
		}

		// Preserve earlier successes while discarding an uncreated addition.
		*task = before;
		self.store
			.record_effect(self.guard, task, 0, EffectState::Settled)?;

		result
	}

	async fn create_participant(
		&mut self,
		task: &mut Task,
		index: usize,
		repository: &crate::Repository,
	) -> Result<(), Problem> {
		let options = task.creation_options(index);
		if !options.no_fetch && repository.remote.is_some() {
			task.participants[index].phase = Phase::Fetching;
			self.observer
				.phase(&task.name, &task.participants[index].name, "fetching")
				.await?;
			self.control.check()?;
			self.store
				.record_effect(self.guard, task, index, EffectState::Pending)?;
			let result = self.git.fetch(repository, self.control).await;
			self.store
				.record_effect(self.guard, task, index, EffectState::Settled)?;
			result?;
		}
		self.observer
			.phase(&task.name, &task.participants[index].name, "creating")
			.await?;
		self.control.check()?;
		self.store.verify()?;
		self.git.revalidate(repository, self.control).await?;
		let plan = self
			.git
			.plan(
				repository,
				&task.name,
				options.checkout,
				task.participants[index].from.as_deref(),
				SourceResolution::Cached,
				self.control,
			)
			.await?;
		self.store
			.validate_destination(&task.participants[index].destination)?;
		task.participants[index].phase = Phase::Creating;
		self.store
			.record_effect(self.guard, task, index, EffectState::Pending)?;
		let result = self
			.git
			.add(
				repository,
				&task.name,
				&task.participants[index].destination,
				&plan,
				self.control,
			)
			.await;
		if result
			.as_ref()
			.is_err_and(|error| error.code == ProblemCode::SupervisionFailed)
		{
			self.uncertain = true;
			return result;
		}

		if result.is_ok() {
			self.changed = true;
		}
		crate::settle_add::settle_add(result, task, index, self.git, self.store, self.guard)
			.await?;
		self.publish(task, index)?;
		self.report(task, index, Kind::Added).await?;
		self.finish_addition(task, index, repository).await
	}

	pub(super) async fn finish_addition(
		&mut self,
		task: &mut Task,
		index: usize,
		repository: &crate::Repository,
	) -> Result<(), Problem> {
		if task.participants[index].phase == Phase::Pulling
			&& self.accept_pull.contains(&task.participants[index].name)
		{
			self.begin(task)?;
			task.participants[index].phase = Phase::Completed;
			self.changed = true;
			self.publish(task, index)?;
			return self.report(task, index, Kind::Updated).await;
		}

		if task.participants[index].phase == Phase::Pulling {
			return Err(Problem::new(
				ProblemCode::GitFailed,
				"The participant has an unsettled pull outcome.",
			)
			.remedy("Inspect git status, complete or abort the pull, then use recover --accept-pull for this participant. Amend never replays an uncertain pull."));
		}

		if task.creation_options(index).pull {
			self.begin(task)?;
			self.observer
				.phase(&task.name, &task.participants[index].name, "pulling")
				.await?;
			self.control.check()?;
			task.participants[index].phase = Phase::Pulling;
			self.store
				.record_effect(self.guard, task, index, EffectState::Pending)?;
			let result = self
				.git
				.pull(
					repository,
					&task.participants[index].destination,
					self.control,
				)
				.await;
			if result
				.as_ref()
				.is_err_and(|error| error.code == ProblemCode::SupervisionFailed)
			{
				self.uncertain = true;
				return result;
			}
			if let Some(failure) =
				crate::optional_pull::settle_pull(result, task, index, self.store, self.guard)?
			{
				self.pull_failures.push(failure);
			}
		}
		self.begin(task)?;
		task.participants[index].phase = Phase::Completed;
		self.changed = true;
		self.publish(task, index)
	}
}
