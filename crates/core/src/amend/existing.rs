//! Update established participants without applying creation options to them.
use super::{Amendment, plan::Change};
use crate::{
	AmendGit, AmendState, AmendmentChangeKind as Kind, Phase, Problem, ProblemCode,
	RemovalObserver, Task,
};

impl<G: AmendGit, S: AmendState, O: RemovalObserver> Amendment<'_, G, S, O> {
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

		let mut participant = current.participant;
		if participant.phase != Phase::Completed {
			self.verify_created(task, &participant).await?;
			participant.phase = Phase::Completed;
		}

		let renaming = participant.name != change.name;
		let updated = renaming || task.participants[index] != participant;
		if renaming {
			self.rename(
				task,
				index,
				&change.repository,
				participant,
				change.name.clone(),
			)
			.await?;
		} else if updated {
			self.begin(task)?;
			task.participants[index] = participant;
			self.changed = true;
			self.publish(task, index)?;
		}

		let already_added = self
			.changes
			.iter()
			.any(|item| item.kind == Kind::Added && item.participant == change.name);
		if updated && !already_added {
			let kind = if renaming {
				Kind::Renamed
			} else {
				Kind::Updated
			};
			self.report(task, index, kind).await?;
		}
		if !updated && !change.repair {
			self.report(task, index, Kind::Unchanged).await?;
		}
		Ok(())
	}

	/// Older additions can still need verification after the Git writer settled.
	async fn verify_created(
		&self,
		task: &Task,
		participant: &crate::Participant,
	) -> Result<(), Problem> {
		let observed = self
			.git
			.reconcile(participant, &task.name, self.control)
			.await?;
		if observed.destination.as_ref() != Some(&participant.destination) {
			return Err(Problem::new(
				ProblemCode::RepositoryChanged,
				"The task branch is no longer at the recorded participant worktree.",
			)
			.remedy(
				"Inspect git worktree list and the participant branch before retrying recovery.",
			));
		}
		if observed.operation_in_progress {
			return Err(Problem::new(
				ProblemCode::ResourceBusy,
				"The participant has native Git locks, unresolved conflicts or an unfinished Git operation.",
			)
			.remedy(
				"Inspect git status and finish or abort the Git operation before retrying recovery.",
			));
		}
		Ok(())
	}
}
