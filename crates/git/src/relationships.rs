//! Reconcile verified linked-worktree identities, including broken location pointers.
mod copy;
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
		self.inspect_relationship(repository, participant, control, false)
			.await
	}

	async fn repair(
		&self,
		repository: &Repository,
		participant: &Participant,
		control: &Control,
	) -> Result<(), Problem> {
		self.repair_links(repository, participant, control, false)
			.await
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
	Problem::new(ProblemCode::RepositoryChanged, message).remedy("Inspect the supplied repository and participant paths; preserve their files and retry after correcting the location.")
}

impl Runner {
	async fn inspect_relationship(
		&self,
		repository: &Repository,
		participant: &Participant,
		control: &Control,
		relocating: bool,
	) -> Result<Relationship, Problem> {
		self.revalidate(repository, control).await?;
		if repository.identity != participant.backing_identity {
			return Err(changed(
				"The supplied repository is not this participant's backing storage.",
			));
		}
		let location = locate(repository, participant)?;
		let blocked = if relocating {
			crate::operations::locks_present(&location.administration, &repository.common_dir)?
		} else {
			operations_present(&location.administration, &repository.common_dir)?
		};
		if blocked {
			return Err(Problem::new(
				ProblemCode::ResourceBusy,
				"The participant has an unfinished Git operation or native lock.",
			)
			.remedy("Finish or abort the affected Git operation, then retry the operation."));
		}
		let observed = self
			.checked(
				&location.destination,
				&args(&["rev-parse", "--path-format=absolute", "--git-common-dir"]),
				control,
			)
			.await;
		control.check()?;
		let common_needs_repair = match observed {
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
		let needs_repair = !location.registration_matches || common_needs_repair;
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

	async fn repair_links(
		&self,
		repository: &Repository,
		participant: &Participant,
		control: &Control,
		relocating: bool,
	) -> Result<(), Problem> {
		let _gate = relationship_lock(repository, control).await?;
		let current = self
			.inspect_relationship(repository, participant, control, relocating)
			.await?;
		let mut arguments = args(&["worktree", "repair", "--"]);
		arguments.push(current.participant.destination.as_os_str().into());
		self.effect(&repository.source, &arguments, control).await?;
		let repaired = self
			.inspect_relationship(repository, participant, &Control::new(|| false), relocating)
			.await?;
		if repaired.needs_repair {
			return Err(changed(
				"Git completed repair but the participant links remain unresolved.",
			));
		}
		Ok(())
	}
}

impl workyard_core::RelocationGit for Runner {
	async fn recovery_registration(
		&self,
		entry: &workyard_core::RelocationParticipant,
		undo: bool,
		previous: Option<&std::path::Path>,
		control: &Control,
	) -> Result<Option<workyard_core::RegisteredCopy>, Problem> {
		copy::displaced(self, entry, undo, previous, control).await
	}
	async fn displaced_copy(
		&self,
		entry: &workyard_core::RelocationParticipant,
		undo: bool,
		control: &Control,
	) -> Result<Option<workyard_core::RegisteredCopy>, Problem> {
		copy::displaced(self, entry, undo, None, control).await
	}
	async fn bind_displaced_copy(
		&self,
		entry: &workyard_core::RelocationParticipant,
		undo: bool,
		registered: &workyard_core::RegisteredCopy,
		control: &Control,
	) -> Result<(), Problem> {
		copy::repair(self, entry, undo, Some(registered), control).await
	}
	async fn copy_is_bound(
		&self,
		entry: &workyard_core::RelocationParticipant,
		undo: bool,
		control: &Control,
	) -> Result<bool, Problem> {
		copy::inspect(self, entry, undo, control).await
	}

	async fn bind_copy(
		&self,
		entry: &workyard_core::RelocationParticipant,
		undo: bool,
		control: &Control,
	) -> Result<(), Problem> {
		copy::repair(self, entry, undo, None, control).await
	}

	async fn relocation_relationship(
		&self,
		participant: &Participant,
		control: &Control,
	) -> Result<Relationship, Problem> {
		let mut repository = None;
		for path in [
			&participant.source,
			&participant.destination,
			&participant.common_dir,
		] {
			if !path
				.try_exists()
				.map_err(|error| Problem::io("Inspect cached repository location", error))?
			{
				continue;
			}
			let mut current = self
				.inspect(path, participant.remote.as_deref(), control)
				.await?;
			if current.identity != participant.backing_identity {
				return Err(changed(
					"Cached location refers to different backing storage.",
				));
			}
			if path == &participant.destination {
				// A linked worktree is a discovery route; its backing storage remains
				// stable while workspace content moves.
				current.source = current.common_dir.clone();
			}
			repository = Some(current);
			break;
		}
		let repository = repository.ok_or_else(|| {
			changed("Backing storage is unreachable; preserve the incomplete participant evidence.")
		})?;
		self.inspect_relationship(&repository, participant, control, true)
			.await
	}
	async fn repair_relocation(
		&self,
		repository: &Repository,
		participant: &Participant,
		control: &Control,
	) -> Result<(), Problem> {
		self.repair_links(repository, participant, control, true)
			.await
	}
}
