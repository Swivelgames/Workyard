use crate::{Phase, Problem, ProblemCode, RecoveryRecord, TaskLifecycle};

pub(super) fn validate(record: &RecoveryRecord, accept_pull: &[String]) -> Result<(), Problem> {
	if let Some(index) = record.pending
		&& record.task.participants[index].phase != Phase::Fetching
	{
		return Err(record.writer_problem(index));
	}

	if record.task.lifecycle != TaskLifecycle::Active
		|| record
			.task
			.removal
			.as_ref()
			.is_some_and(|removal| !removal.restoration_complete())
	{
		return Err(Problem::new(
			ProblemCode::InvalidInput,
			"Amend requires an active task without an incomplete removal.",
		));
	}

	for accepted in accept_pull {
		if !record
			.task
			.participants
			.iter()
			.any(|p| &p.name == accepted && p.phase == Phase::Pulling)
		{
			return Err(Problem::new(
				ProblemCode::InvalidInput,
				"--accept-pull must name a participant with an unsettled pull.",
			));
		}
	}

	Ok(())
}

/// A stored request is a locator recipe, not authority to replace an existing repository.
pub(super) async fn refresh_request(
	request: &mut crate::AmendRequest,
	task: &crate::Task,
	git: &impl crate::AmendGit,
	control: &crate::Control,
) -> Result<(), Problem> {
	for input in &mut request.repositories {
		let Some(participant) = task
			.participants
			.iter()
			.find(|participant| participant.source == input.path)
		else {
			continue;
		};
		input.path = locate(participant, git, control).await?;
	}

	Ok(())
}

pub(super) async fn locate(
	participant: &crate::Participant,
	git: &impl crate::AmendGit,
	control: &crate::Control,
) -> Result<std::path::PathBuf, Problem> {
	let mut failure = None;
	let mut located = None;
	for path in [
		&participant.source,
		&participant.destination,
		&participant.common_dir,
	] {
		control.check()?;
		match git
			.inspect(path, participant.remote.as_deref(), control)
			.await
		{
			Ok(repository) => {
				if repository.identity != participant.backing_identity {
					return Err(Problem::new(
						ProblemCode::RepositoryChanged,
						"Recorded amendment location now points to different backing storage.",
					));
				}
				located = Some(repository.source);
				break;
			}
			Err(problem) => failure = Some(problem),
		}
	}
	located.ok_or_else(|| failure.expect("all repository locators inspected"))
}
