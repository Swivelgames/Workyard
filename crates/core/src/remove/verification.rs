//! Verify retained worktree facts for restoration and unchanged recovery.
use crate::{
	Control, Problem, ProblemCode, RecoveryRecord, RemovalDirection, RemovalGit, RemovalPhase,
	Task, TaskLifecycle,
};

pub(super) async fn unchanged(
	record: &RecoveryRecord,
	task: &mut Task,
	git: &impl RemovalGit,
	control: &Control,
) -> Result<bool, Problem> {
	let Some(removal) = &task.removal else {
		return Ok(false);
	};
	if task.lifecycle == TaskLifecycle::Retired {
		return Ok(removal
			.participants
			.iter()
			.all(|participant| participant.phase == RemovalPhase::Removed)
			&& record.matches_published(task));
	}
	if removal.direction != RemovalDirection::Restore
		|| removal.participants.iter().any(|participant| {
			!matches!(
				participant.phase,
				RemovalPhase::Present | RemovalPhase::Restored
			)
		}) {
		return Ok(false);
	}

	for current in &mut task.participants {
		participant(current, git, control).await?;
	}
	Ok(record.matches_published(task))
}

pub(super) async fn participant(
	participant: &mut crate::Participant,
	git: &impl RemovalGit,
	control: &Control,
) -> Result<(), Problem> {
	control.check()?;
	let observed = git.reconcile_restored(participant, control).await?;
	if observed.operation_in_progress {
		return Err(Problem::new(
			ProblemCode::ResourceBusy,
			"Surviving worktree has an unfinished Git operation; inspect Git before recovery.",
		));
	}
	let destination = observed.destination.ok_or_else(|| {
		Problem::new(
			ProblemCode::RepositoryChanged,
			"A surviving worktree is missing; inspect Git before retrying recovery.",
		)
	})?;
	participant.source = observed.repository.source;
	participant.common_dir = observed.repository.common_dir;
	participant.destination = destination;
	Ok(())
}
