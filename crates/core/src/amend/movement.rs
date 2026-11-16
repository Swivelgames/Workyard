//! Settle a participant move before observing cancellation or delivering progress.
use super::Amendment;
use crate::{
	AmendGit, AmendState, EffectState, Observer, Participant, Problem, ProblemCode, Repository,
	Task,
};

impl<G: AmendGit, S: AmendState, O: Observer> Amendment<'_, G, S, O> {
	pub(super) async fn rename(
		&mut self,
		task: &mut Task,
		index: usize,
		repository: &Repository,
		mut participant: Participant,
		name: String,
	) -> Result<(), Problem> {
		let destination = task.root.join(&task.name).join(&name);
		if participant.destination == destination {
			self.begin(task)?;
			participant.name = name;
			task.participants[index] = participant;
			self.changed = true;
			return self.publish(task, index);
		}

		self.store.validate_destination(&destination)?;
		self.begin(task)?;
		self.store
			.record_effect(self.guard, task, index, EffectState::Pending)?;
		let result = self
			.git
			.move_worktree(repository, &participant, &destination, self.control)
			.await;
		if result
			.as_ref()
			.is_err_and(|problem| problem.code == ProblemCode::SupervisionFailed)
		{
			self.uncertain = true;
			return result;
		}
		if result.is_err() {
			self.inspect_failed_move(task, index, repository, &participant)
				.await?;
			return result;
		}

		participant.destination = destination;
		participant.name = name;
		task.participants[index] = participant;
		self.changed = true;
		self.publish(task, index)
	}

	async fn inspect_failed_move(
		&mut self,
		task: &mut Task,
		index: usize,
		repository: &Repository,
		previous: &Participant,
	) -> Result<(), Problem> {
		// Nonzero Git exit does not establish that the directory stayed put.
		let settled = crate::Control::new(|| false);
		match self.git.relationship(repository, previous, &settled).await {
			Ok(observed) if observed.participant.destination != previous.destination => {
				task.participants[index] = observed.participant;
				self.changed = true;
			}
			Ok(_) => {}
			Err(_) => self.uncertain = true,
		}

		self.store
			.record_effect(self.guard, task, index, EffectState::Settled)
	}
}
