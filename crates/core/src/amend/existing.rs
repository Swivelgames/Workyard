//! Update established participants without applying creation options to them.
use super::{Amendment, plan::Change};
use crate::{
	AmendGit, AmendState, AmendmentChangeKind as Kind, Observer, Phase, Problem, ProblemCode, Task,
};

impl<G: AmendGit, S: AmendState, O: Observer> Amendment<'_, G, S, O> {
	pub(super) async fn existing(
		&mut self,
		task: &mut Task,
		change: Change,
	) -> Result<(), Problem> {
		let index = change.existing.expect("existing participant");
		let current = self
			.git
			.relationship(&change.repository, &task.participants[index], self.control)
			.await?;
		if current.needs_repair {
			return Err(Problem::new(
				ProblemCode::RepositoryChanged,
				"Git links changed after amendment validation; retry amend.",
			));
		}

		let participant = current.participant;
		let renaming = participant.name != change.name;
		let updated = renaming || task.participants[index] != participant;
		if renaming {
			self.rename(task, index, &change.repository, participant, change.name)
				.await?;
		} else if updated {
			self.begin(task)?;
			task.participants[index] = participant;
			self.changed = true;
			self.publish(task, index)?;
		}

		if updated {
			let kind = if renaming {
				Kind::Renamed
			} else {
				Kind::Updated
			};
			self.report(task, index, kind).await?;
		}
		if task.participants[index].phase != Phase::Completed {
			return self.finish_addition(task, index, &change.repository).await;
		}
		if !updated && !change.repair {
			self.report(task, index, Kind::Unchanged).await?;
		}
		Ok(())
	}
}
