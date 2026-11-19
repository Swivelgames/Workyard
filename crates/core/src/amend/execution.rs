use super::{Amendment, plan::Change};
use crate::{
	AmendGit, AmendState, AmendmentChangeKind as Kind, EffectState, Observer, Problem, ProblemCode,
	Task,
};

impl<G: AmendGit, S: AmendState, O: Observer> Amendment<'_, G, S, O> {
	pub(super) async fn execute(
		&mut self,
		task: &mut Task,
		changes: Vec<Change>,
	) -> Result<(), Problem> {
		for change in changes.iter().filter(|change| change.repair) {
			self.repair(task, change).await?;
		}

		for change in changes {
			self.control.check()?;
			if change.addition {
				self.add(task, change).await?;
			} else {
				self.existing(task, change).await?;
			}
		}

		Ok(())
	}

	async fn repair(&mut self, task: &mut Task, change: &Change) -> Result<(), Problem> {
		let index = change.existing.expect("existing repair participant");
		self.begin(task)?;
		self.store
			.record_effect(self.guard, task, index, EffectState::Pending)?;
		let result = self
			.git
			.repair(&change.repository, &task.participants[index], self.control)
			.await;
		if result
			.as_ref()
			.is_err_and(|error| error.code == ProblemCode::SupervisionFailed)
		{
			self.uncertain = true;
			return result;
		}
		// Repair may fix other links even when Git reports a later failure.
		self.changed = true;
		if result.is_ok() {
			task.participants[index] = change.participant.clone();
			task.participants[index].placement = None;
		}
		self.publish(task, index)?;
		result?;
		self.report(task, index, Kind::Repaired).await
	}
}
