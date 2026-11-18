//! Workspace relocation policy. Adapters own native transfer and durable authority.
mod copy;
mod ports;
use crate::{Control, Problem, ProblemCode, Repository};
pub use ports::*;

pub async fn relocate(
	state: &mut impl RelocationState,
	git: &impl RelocationGit,
	control: &Control,
	undo: bool,
	observer: &impl crate::Observer,
) -> Result<(), Problem> {
	control.check()?;
	state.verify()?;
	if state.activated() {
		if undo {
			return Err(Problem::new(
				ProblemCode::InvalidInput,
				"Relocation is already active. Returning requires a new relocation.",
			));
		}
		return state.cleanup();
	}
	if state.pending_writer() {
		let writer = state
			.pending_writer_description()
			.unwrap_or_else(|| "its managed Git writer".into());
		return Err(Problem::new(ProblemCode::ResourceBusy,
			format!("Relocation has no durable proof that {writer} stopped."))
			.remedy("Inspect the interrupted Workyard and Git processes. A free workspace gate does not establish writer settlement; preserve the relocation evidence."));
	}

	observer.phase("", "", "preparing").await?;
	let mut participants = state.participants();
	// Resolve current Git-owned locations before recording relocation intent.
	for entry in participants.iter_mut().filter(|_| !state.started()) {
		if entry.original.worktree_identity.is_none()
			&& !entry
				.original
				.destination
				.try_exists()
				.map_err(|error| Problem::io("Inspect incomplete participant location", error))?
		{
			continue;
		}
		match git.relocation_relationship(&entry.original, control).await {
			Ok(current) => {
				if current.participant.destination != entry.original.destination {
					// Preserve an externally repaired location rather than normalizing it.
					entry.destination = state.relocated_path(&current.participant.destination)?;
				}
				entry.original = current.participant;
			}
			Err(problem)
				if problem.code == ProblemCode::RepositoryChanged
					&& !entry
						.original
						.destination
						.try_exists()
						.map_err(|error| Problem::io("Inspect missing participant", error))? => {}
			Err(problem) => return Err(problem),
		}
	}
	let phase = match (state.is_copy(), undo) {
		(false, _) => "moving_workspace",
		(true, false) => "copying_workspace",
		(true, true) => "restoring_workspace",
	};
	observer.phase("", "", phase).await?;
	if state.is_copy() {
		copy::transfer(state, git, &participants, undo, control, observer).await?;
	} else {
		state.begin(participants.clone())?;
		state.transfer(undo, control)?;
		observer.phase("", "", "repairing_git").await?;
		repair_moved_worktrees(state, git, participants, undo, control).await?;
	}
	control.check()?;
	state.verify()?;
	observer.phase("", "", "transferring_state").await?;
	state.activate(undo)?;
	let phase = if state.is_copy() {
		"marking_snapshot"
	} else {
		"cleaning_source"
	};
	observer.phase("", "", phase).await?;
	state.cleanup()
}

async fn repair_moved_worktrees(
	state: &mut impl RelocationState,
	git: &impl RelocationGit,
	participants: Vec<RelocationParticipant>,
	undo: bool,
	control: &Control,
) -> Result<(), Problem> {
	for entry in participants {
		control.check()?;
		state.verify()?;
		let mut participant = entry.original;
		participant.destination = if undo {
			participant.destination
		} else {
			entry.destination
		};
		// Missing/unstarted or already removed participants retain their evidence.
		if participant.worktree_identity.is_none()
			|| !participant
				.destination
				.try_exists()
				.map_err(|error| Problem::io("Inspect participant destination", error))?
		{
			continue;
		}
		let repository = Repository {
			source: participant.source.clone(),
			common_dir: participant.common_dir.clone(),
			remote: participant.remote.clone(),
			identity: participant.backing_identity.clone(),
		};
		let current = git.relocation_relationship(&participant, control).await?;
		if !current.needs_repair && current.participant.destination == participant.destination {
			continue;
		}
		state.writer(true)?;
		let result = git
			.repair_relocation(&repository, &participant, control)
			.await;
		if result
			.as_ref()
			.is_err_and(|problem| problem.code == ProblemCode::SupervisionFailed)
		{
			return result;
		}
		state.writer(false)?;
		result?;
		let verified = git
			.relocation_relationship(&participant, &Control::new(|| false))
			.await?;
		if verified.needs_repair || verified.participant.destination != participant.destination {
			return Err(Problem::new(
				ProblemCode::RepositoryChanged,
				"Git repair did not establish the requested participant location.",
			));
		}
	}
	Ok(())
}
