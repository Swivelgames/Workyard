//! Forward removal discloses the whole task before it begins unlinking participants.
use super::{RemovalRun, consent};
use crate::{
	EffectState, OperationOutcome, Problem, ProblemCode, RecoveryState, Removal, RemovalDirection,
	RemovalGit, RemovalObservation, RemovalObserver, RemovalParticipant, RemovalPhase, Task,
	TaskLifecycle,
};

// Keep the original task index when recovery skips a previously removed participant.
struct InspectedParticipant {
	index: usize,
	observation: RemovalObservation,
}

impl<G: RemovalGit, S: RecoveryState, O: RemovalObserver> RemovalRun<'_, G, S, O> {
	pub(super) async fn retire(&mut self, task: &mut Task) -> Result<(), Problem> {
		self.observer.begin_disclosure(task, None);
		let inspected = self.inspect_participants(task).await?;
		self.inspected = true;

		let discards_files = inspected
			.iter()
			.any(|member| !member.observation.dirty_paths.is_empty());
		consent::authorize(
			self.observer,
			self.options,
			&task.name,
			None,
			discards_files,
		)
		.await?;
		if task.removal.is_none() {
			task.removal = Some(removal_intent(&inspected));
		}

		for member in inspected {
			self.unlink_participant(task, member.index, member.observation)
				.await?;
		}

		task.lifecycle = TaskLifecycle::Retired;
		task.operation.outcome = OperationOutcome::Succeeded;
		task.removal.as_mut().expect("removal intent").retired_at = Some(retirement_timestamp()?);
		self.publish(task, 0)?;
		self.store.cleanup_retired(self.guard, task)
	}

	async fn inspect_participants(
		&self,
		task: &Task,
	) -> Result<Vec<InspectedParticipant>, Problem> {
		let mut inspected = Vec::new();
		for (index, participant) in task.participants.iter().enumerate() {
			self.control.check()?;
			if task
				.removal
				.as_ref()
				.is_some_and(|r| r.participants[index].phase == RemovalPhase::Removed)
			{
				continue;
			}
			let observation = self.git.inspect_removal(participant, self.control).await?;
			self.observer
				.disclose(&participant.name, &observation)
				.await?;
			inspected.push(InspectedParticipant { index, observation });
		}

		Ok(inspected)
	}

	async fn unlink_participant(
		&mut self,
		task: &mut Task,
		index: usize,
		mut disclosed: RemovalObservation,
	) -> Result<(), Problem> {
		self.control.check()?;
		self.store.verify()?;

		let participant = task.participants[index].clone();
		let result = consent::unlink(
			self.git,
			self.observer,
			self.control,
			&participant,
			&mut disclosed,
			|current| {
				self.control.check()?;

				task.participants[index].destination =
					current.destination.clone().expect("verified destination");
				let entry = &mut task.removal.as_mut().expect("removal intent").participants[index];
				entry.branch.clone_from(&current.branch);
				entry.dirty_at_start |= !current.dirty_paths.is_empty();

				self.publication_attempted = true;
				self.store
					.record_effect(self.guard, task, index, EffectState::Pending)
			},
		)
		.await;

		match result {
			Ok(()) => {
				task.removal.as_mut().expect("removal intent").participants[index].phase =
					RemovalPhase::Removed;
				self.publish(task, index)?;
			}
			Err(crate::UnlinkFailure::BeforeLaunch(problem)) => return Err(problem),
			Err(crate::UnlinkFailure::Command(problem)) => {
				if problem.code != ProblemCode::SupervisionFailed {
					self.publish(task, index)?;
				}
				return Err(problem);
			}
		}

		self.observer
			.removed(&task.name, &task.participants[index].name)
			.await
	}
}

// A new removal inspects the whole task before creating its initial progress record.
fn removal_intent(inspected: &[InspectedParticipant]) -> Removal {
	let participants = inspected
		.iter()
		.map(|member| RemovalParticipant {
			branch: member.observation.branch.clone(),
			dirty_at_start: !member.observation.dirty_paths.is_empty(),
			phase: RemovalPhase::Present,
		})
		.collect();

	Removal {
		direction: RemovalDirection::Remove,
		retired_at: None,
		participants,
	}
}

fn retirement_timestamp() -> Result<u64, Problem> {
	let elapsed = std::time::SystemTime::now()
		.duration_since(std::time::UNIX_EPOCH)
		.map_err(|_| Problem::new(ProblemCode::IoError, "System clock precedes Unix epoch."))?;
	elapsed
		.as_millis()
		.try_into()
		.map_err(|_| Problem::new(ProblemCode::IoError, "Retirement timestamp overflow."))
}
