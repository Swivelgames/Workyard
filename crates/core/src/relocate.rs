//! Workspace relocation policy. Adapters own native transfer and durable authority.
use crate::{Control, Participant, Problem, ProblemCode, Repository};
use std::path::PathBuf;

#[derive(Clone, Debug)]
pub struct RelocationParticipant {
	pub task: String,
	pub original: Participant,
	pub destination: PathBuf,
}

pub trait RelocationGit {
	fn relocation_relationship(
		&self,
		participant: &Participant,
		control: &Control,
	) -> impl std::future::Future<Output = Result<crate::Relationship, Problem>> + Send;
	fn repair_relocation(
		&self,
		repository: &Repository,
		participant: &Participant,
		control: &Control,
	) -> impl std::future::Future<Output = Result<(), Problem>> + Send;
}

pub trait RelocationState {
	fn participants(&self) -> Vec<RelocationParticipant>;
	fn activated(&self) -> bool;
	fn started(&self) -> bool;
	fn relocated_path(&self, path: &std::path::Path) -> Result<PathBuf, Problem>;
	fn verify(&self) -> Result<(), Problem>;
	fn begin(&mut self, participants: Vec<RelocationParticipant>) -> Result<(), Problem>;
	fn transfer(&mut self, undo: bool, control: &Control) -> Result<(), Problem>;
	fn pending_writer(&self) -> bool;
	fn writer(&mut self, pending: bool) -> Result<(), Problem>;
	fn activate(&mut self, undo: bool) -> Result<(), Problem>;
	fn cleanup(&mut self) -> Result<(), Problem>;
}

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
		return Err(Problem::new(ProblemCode::ResourceBusy,
			"Relocation has no durable proof that its managed Git writer stopped.")
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
	observer.phase("", "", "moving_workspace").await?;
	state.begin(participants.clone())?;
	state.transfer(undo, control)?;
	observer.phase("", "", "repairing_git").await?;
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
	control.check()?;
	state.verify()?;
	observer.phase("", "", "transferring_state").await?;
	state.activate(undo)?;
	observer.phase("", "", "cleaning_source").await?;
	state.cleanup()
}
