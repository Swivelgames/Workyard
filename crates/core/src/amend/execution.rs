use super::{Amendment, plan::Change};
use crate::{
	AmendGit, AmendState, AmendmentChangeKind as Kind, EffectState, Problem, ProblemCode,
	RemovalObserver, Task,
};

impl<G: AmendGit, S: AmendState, O: RemovalObserver> Amendment<'_, G, S, O> {
	pub(super) async fn execute(
		&mut self,
		task: &mut Task,
		changes: Vec<Change>,
		report_settled: bool,
	) -> Result<(), Problem> {
		let removals = self.inspect_removals(task, report_settled).await?;
		let schedule =
			super::replacement::Schedule::prepare(task, changes, !removals.is_empty(), self.store)?;
		for change in schedule
			.before
			.iter()
			.chain(&schedule.after)
			.filter(|change| change.repair)
		{
			self.repair(task, change).await?;
		}

		for change in schedule.before {
			self.apply(task, change).await?;
		}
		for (target, observation) in removals {
			self.unlink(task, &target, observation).await?;
		}
		for change in schedule.after {
			self.apply(task, change).await?;
		}
		Ok(())
	}

	async fn apply(&mut self, task: &mut Task, mut change: Change) -> Result<(), Problem> {
		self.control.check()?;
		// Unlinking compacts membership; an index from the initial plan is now stale.
		change.existing = task
			.participants
			.iter()
			.position(|p| p.backing_identity == change.repository.identity);
		if change.addition {
			return self.add(task, change).await;
		}
		self.existing(task, change).await
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
