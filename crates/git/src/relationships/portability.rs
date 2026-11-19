//! A preview binds consent to the inspected repository, locations and native entries.
use super::{changed, location, relationship_lock};
use crate::{Runner, operations::locks_present};
use std::{
	collections::HashSet,
	path::{Path, PathBuf},
};
use workyard_core::{Control, Participant, PlacementHint, Problem, ProblemCode, Repository};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Placement {
	pub registered: PathBuf,
	pub destination: PathBuf,
	pub competing: bool,
	pub needs_repair: bool,
	pub common_dir: PathBuf,
	pub source: PathBuf,
	backing: String,
	admin: String,
	local_identity: (u64, u64),
	registered_identity: Option<(u64, u64)>,
}

impl Placement {
	pub fn diagnostic(&self) -> Problem {
		placement_problem(&self.registered, &self.destination, self.competing)
	}

	pub fn update_hint(&self, participant: &mut Participant, workspace: &Path) {
		participant.common_dir = self.common_dir.clone();
		participant.source = self.source.clone();
		participant.destination = self.destination.clone();
		participant.placement = Some(PlacementHint {
			source: self.source.clone(),
			common_dir: self.common_dir.clone(),
			destination: self.destination.clone(),
			workspace: workspace.to_owned(),
			worktree_entry: Some(self.local_identity),
		});
	}
}

impl Runner {
	pub async fn inspect_placement(
		&self,
		participant: &Participant,
		project: Option<&Path>,
		control: &Control,
	) -> Result<Placement, Problem> {
		let repository = self
			.placement_repository(participant, project, control)
			.await?;
		self.preview_in(&repository, participant, control).await
	}

	pub(crate) async fn placement_repository(
		&self,
		participant: &Participant,
		project: Option<&Path>,
		control: &Control,
	) -> Result<Repository, Problem> {
		let former = participant
			.placement
			.as_ref()
			.map(|h| h.common_dir.as_path());
		let paths = [
			Some(participant.source.as_path()),
			Some(participant.common_dir.as_path()),
			project,
			former,
			Some(participant.destination.as_path()),
		];
		for path in paths.into_iter().flatten() {
			control.check()?;
			if !path
				.try_exists()
				.map_err(|e| Problem::io("Inspect backing location", e))?
			{
				continue;
			}
			let Ok(mut repo) = crate::Repository::local(self, path, control).await else {
				control.check()?;
				continue;
			};
			if repo.identity != participant.backing_identity {
				continue;
			}
			if super::pointer::same_path(path, &participant.destination) {
				repo.source = repo.common_dir.clone();
			}
			return Ok(repo.into());
		}
		Err(changed(
			"The recorded backing repository is unavailable. Track its new location and retry workyard recover.",
		))
	}

	async fn preview_in(
		&self,
		repo: &Repository,
		participant: &Participant,
		control: &Control,
	) -> Result<Placement, Problem> {
		let link = location::inspect(repo, participant)?;
		let admin = workyard_system::directory_identity(&link.administration)?;
		if participant.worktree_identity.as_ref() != Some(&admin) {
			return Err(changed("Linked worktree identity cannot be verified."));
		}
		let common_matches = self.uses_backing(&link.destination, repo, control).await?;
		let local_identity = workyard_system::entry_identity(&link.destination)?;
		let registered_identity = if link.competing {
			Some(workyard_system::entry_identity(&link.registered)?)
		} else {
			None
		};
		Ok(Placement {
			registered: link.registered,
			destination: link.destination,
			competing: link.competing,
			needs_repair: !link.registration_matches || !common_matches,
			common_dir: repo.common_dir.clone(),
			source: repo.source.clone(),
			backing: repo.identity.clone(),
			admin,
			local_identity,
			registered_identity,
		})
	}

	pub async fn apply_placement(
		&self,
		participant: &Participant,
		preview: &Placement,
		activate: bool,
		unavailable_worktrees: &HashSet<String>,
		control: &Control,
	) -> Result<(), Problem> {
		let repo: Repository = crate::Repository::local(self, &preview.source, control)
			.await?
			.into();
		if repo.identity != preview.backing || participant.backing_identity != preview.backing {
			return Err(changed("Backing storage changed since inspection."));
		}
		let _gate = relationship_lock(&repo, control).await?;
		crate::Repository::from(&repo)
			.revalidate_local(self, control)
			.await?;
		let current = self.preview_in(&repo, participant, control).await?;
		// Git can reconnect multiple links after a main clone moves. Accept a
		// peer already repaired to the exact worktree named by this preview.
		if !current.needs_repair
			&& current.destination == preview.destination
			&& current.admin == preview.admin
			&& current.local_identity == preview.local_identity
		{
			return Ok(());
		}
		if &current != preview {
			return Err(changed(
				"Worktree locations changed since inspection. Run workyard recover again.",
			));
		}
		if !current.needs_repair {
			return Ok(());
		}
		if current.competing && !activate {
			return Err(current.diagnostic());
		}
		let link = location::inspect(&repo, participant)?;
		if locks_present(&link.administration, &repo.common_dir)? {
			return Err(Problem::new(
				ProblemCode::ResourceBusy,
				"A Git writer is active. Retry recovery when it finishes.",
			));
		}
		control.check()?;
		self.repair_registered_tree(
			&repo,
			participant,
			&link.administration,
			&current.destination,
			Some(unavailable_worktrees),
			control,
		)
		.await?;
		let repaired = self
			.preview_in(&repo, participant, &Control::new(|| false))
			.await?;
		if repaired.needs_repair
			|| repaired.destination != current.destination
			|| repaired.local_identity != current.local_identity
		{
			return Err(changed(
				"Git completed repair but the verified worktree is not registered here.",
			));
		}
		Ok(())
	}
}

pub(super) fn placement_problem(registered: &Path, candidate: &Path, competing: bool) -> Problem {
	let message = format!("Git registers {registered:?}; workspace files are at {candidate:?}.");
	let (code, remedy) = if competing {
		(
			ProblemCode::DecisionRequired,
			"Run workyard recover in the workspace you want to use.",
		)
	} else {
		(
			ProblemCode::RepositoryChanged,
			"Run workyard recover to repair the worktree link.",
		)
	};
	let mut problem = Problem::new(code, message).remedy(remedy);
	problem.worktree_locations = Some(Box::new(workyard_core::WorktreeLocations {
		registered: registered.into(),
		candidate: candidate.into(),
	}));
	problem
}
