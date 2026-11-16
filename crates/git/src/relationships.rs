//! Reconcile verified linked-worktree identities, including broken location pointers.
mod location;
use crate::{Runner, operations::operations_present, output::path_line, process::args};
use location::locate;
use std::path::Path;
use workyard_core::{
	AmendGit, Control, Git, Participant, Problem, ProblemCode, Relationship, Repository,
};

impl AmendGit for Runner {
	async fn relationship(
		&self,
		repository: &Repository,
		participant: &Participant,
		control: &Control,
	) -> Result<Relationship, Problem> {
		self.revalidate(repository, control).await?;
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
			.remedy("Finish or abort the affected Git operation, then retry amend."));
		}
		let observed = self
			.checked(
				&location.destination,
				&args(&["rev-parse", "--path-format=absolute", "--git-common-dir"]),
				control,
			)
			.await;
		control.check()?;
		let needs_repair = match observed {
			Ok(output) => {
				path_line(&output)
					.map_err(|_| changed("Invalid native Git path."))?
					.canonicalize()
					.ok()
					.as_ref() != Some(&repository.common_dir)
			}
			Err(problem) if problem.code == ProblemCode::GitFailed => true,
			Err(problem) => return Err(problem),
		};
		let mut current = participant.clone();
		current.worktree_identity = Some(workyard_system::directory_identity(
			&location.administration,
		)?);
		current.source = repository.source.clone();
		current.common_dir = repository.common_dir.clone();
		current.destination = location.destination;
		Ok(Relationship {
			participant: current,
			needs_repair,
		})
	}

	async fn repair(
		&self,
		repository: &Repository,
		participant: &Participant,
		control: &Control,
	) -> Result<(), Problem> {
		let _gate = relationship_lock(repository, control).await?;
		let current = self.relationship(repository, participant, control).await?;
		let mut arguments = args(&["worktree", "repair", "--"]);
		arguments.push(current.participant.destination.as_os_str().into());
		self.effect(&repository.source, &arguments, control).await?;
		let repaired = self
			.relationship(repository, participant, &Control::new(|| false))
			.await?;
		if repaired.needs_repair {
			return Err(changed(
				"Git completed repair but the participant links remain unresolved.",
			));
		}
		Ok(())
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
		if current.needs_repair || current.participant.destination != participant.destination {
			return Err(changed(
				"Participant location changed before its rename; retry amend.",
			));
		}
		let mut arguments = args(&["worktree", "move", "--"]);
		arguments.push(participant.destination.as_os_str().into());
		arguments.push(destination.as_os_str().into());
		self.effect(&repository.source, &arguments, control).await?;
		let moved = self
			.relationship(repository, participant, &Control::new(|| false))
			.await?;
		if moved.needs_repair || moved.participant.destination != destination {
			return Err(changed(
				"Git moved the worktree, but its registered destination differs from the requested path.",
			));
		}
		Ok(())
	}
}

async fn relationship_lock(
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
	Problem::new(ProblemCode::RepositoryChanged, message).remedy("Inspect the supplied repository and participant paths; preserve their files and retry amend after correcting the location.")
}
