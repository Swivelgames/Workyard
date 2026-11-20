use super::{Amendment, plan::Change};
use crate::{
	AmendGit, AmendState, AmendmentChangeKind as Kind, EffectState, Phase, Problem, ProblemCode,
	RemovalObserver, SourceResolution, Task,
};

impl<G: AmendGit, S: AmendState, O: RemovalObserver> Amendment<'_, G, S, O> {
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
			.create_participant(task, index, &change.repository, &change.name)
			.await;
		let observed_worktree = task.participants[index].worktree_identity.is_some()
			|| matches!(
				task.participants[index].phase,
				Phase::WorktreeCreated | Phase::Completed
			);
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
		name: &str,
	) -> Result<(), Problem> {
		let options = task.creation_options(index);
		if !options.no_fetch && repository.remote.is_some() {
			task.participants[index].phase = Phase::Fetching;
			self.observer.phase(&task.name, name, "fetching").await?;
			self.control.check()?;
			self.store
				.record_effect(self.guard, task, index, EffectState::Pending)?;
			let result = self.git.fetch(repository, self.control).await;
			self.store
				.record_effect(self.guard, task, index, EffectState::Settled)?;
			result?;
		}
		self.observer.phase(&task.name, name, "creating").await?;
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
		self.store.save(self.guard, task)?;
		self.report_named(task, name, Kind::Added).await
	}
}
