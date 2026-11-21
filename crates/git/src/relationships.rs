//! Reconcile verified linked-worktree identities, including broken location pointers.
mod location;
pub(crate) mod pointer;
mod portability;
mod repair;
use crate::{
	Runner, operations::operations_present, output::path_line, process::args,
	worktree_path::same_path,
};
use location::locate;
pub use portability::Placement;
use std::path::Path;
use workyard_core::{
	AmendGit, Control, Participant, Problem, ProblemCode, Relationship, Repository,
};

impl AmendGit for Runner {
	async fn relationship(
		&self,
		repository: &Repository,
		participant: &Participant,
		control: &Control,
	) -> Result<Relationship, Problem> {
		self.inspect_relationship(repository, participant, control)
			.await
	}

	async fn repair(
		&self,
		repository: &Repository,
		participant: &Participant,
		control: &Control,
	) -> Result<(), Problem> {
		self.repair_links(repository, participant, control).await
	}

	async fn move_worktree(
		&self,
		repository: &Repository,
		participant: &Participant,
		destination: &Path,
		control: &Control,
	) -> Result<(), Problem> {
		let _gate = relationship_lock(repository, control).await?;
		let current = self.relationship(repository, participant, control).await?;
		if current.needs_repair
			|| !same_path(&current.participant.destination, &participant.destination)
		{
			return Err(changed(
				"Participant location changed before its rename; retry amend.",
			));
		}
		let mut arguments = args(&["worktree", "move", "--"]);
		arguments.push(dunce::simplified(&participant.destination).into());
		arguments.push(dunce::simplified(destination).into());
		self.effect(&repository.source, &arguments, control).await?;
		let moved = self
			.relationship(repository, participant, &Control::new(|| false))
			.await?;
		if moved.needs_repair || !same_path(&moved.participant.destination, destination) {
			return Err(changed(
				"Git moved the worktree, but its registered destination differs from the requested path.",
			));
		}
		Ok(())
	}
}

pub(crate) async fn relationship_lock(
	repository: &Repository,
	control: &Control,
) -> Result<workyard_system::Lock, Problem> {
	let directory = repository.common_dir.join("workyard");
	std::fs::create_dir_all(&directory)
		.map_err(|error| Problem::io("Create repository coordination directory", error))?;
	let lock = workyard_system::Lock::wait(&directory.join("relationships.lock"), control).await?;
	lock.verify()?;
	Ok(lock)
}

pub(super) fn changed(message: &str) -> Problem {
	Problem::new(ProblemCode::RepositoryChanged, message).remedy("Inspect the supplied repository and participant paths; preserve their files and retry after correcting the location.")
}

impl Runner {
	async fn inspect_relationship(
		&self,
		repository: &Repository,
		participant: &Participant,
		control: &Control,
	) -> Result<Relationship, Problem> {
		crate::Repository::from(repository)
			.revalidate_local(self, control)
			.await?;
		if repository.identity != participant.backing_identity {
			return Err(changed(
				"The supplied repository is not this participant's backing storage.",
			));
		}
		let location = locate(repository, participant)?;
		if operations_present(&location.administration, &repository.common_dir)? {
			return Err(Problem::new(
				ProblemCode::ResourceBusy,
				"The participant has an unfinished Git operation or native lock.",
			)
			.remedy("Finish or abort the affected Git operation, then retry the operation."));
		}
		let common_matches = self
			.uses_backing(&location.destination, repository, control)
			.await?;
		let needs_repair = !location.registration_matches || !common_matches;
		let mut current = participant.clone();
		current.worktree_identity = Some(workyard_system::directory_identity(
			&location.administration,
		)?);
		current.source = repository.source.clone();
		current.common_dir = repository.common_dir.clone();
		if !needs_repair && !same_path(&participant.destination, &location.destination) {
			current.placement = None;
		}
		current.destination = location.destination;
		Ok(Relationship {
			participant: current,
			needs_repair,
		})
	}

	async fn uses_backing(
		&self,
		destination: &Path,
		repository: &Repository,
		control: &Control,
	) -> Result<bool, Problem> {
		let args = args(&["rev-parse", "--path-format=absolute", "--git-common-dir"]);
		let observed = self.checked(destination, &args, control).await;
		control.check()?;
		let bytes = match observed {
			Ok(bytes) => bytes,
			Err(problem) if problem.code == ProblemCode::GitFailed => return Ok(false),
			Err(problem) => return Err(problem),
		};
		let common = path_line(&bytes).map_err(|_| changed("Invalid native Git path."))?;
		Ok(workyard_system::directory_identity(&common)? == repository.identity)
	}

	async fn repair_links(
		&self,
		repository: &Repository,
		participant: &Participant,
		control: &Control,
	) -> Result<(), Problem> {
		let _gate = relationship_lock(repository, control).await?;
		let current = self
			.inspect_relationship(repository, participant, control)
			.await?;
		let location = locate(repository, participant)?;
		self.repair_registered_tree(
			repository,
			participant,
			&location.administration,
			&current.participant.destination,
			None,
			control,
		)
		.await?;
		let repaired = self
			.inspect_relationship(repository, participant, &Control::new(|| false))
			.await?;
		if repaired.needs_repair {
			return Err(changed(
				"Git completed repair but the participant links remain unresolved.",
			));
		}
		Ok(())
	}
}
