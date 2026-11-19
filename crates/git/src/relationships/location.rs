//! Locate a recorded administrative identity without requiring working .git pointers.
use super::changed;
use super::pointer::{absolute, matches_pointer, read_path, same_path};
use std::path::{Path, PathBuf};
use workyard_core::{Participant, Problem, Repository};

struct Registration {
	tree: PathBuf,
	bound: bool,
	current_backlink: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct LinkedLocation {
	pub registered: PathBuf,
	pub competing: bool,
	pub administration: PathBuf,
	pub destination: PathBuf,
	pub registration_matches: bool,
}

pub(super) fn locate(
	repository: &Repository,
	participant: &Participant,
) -> Result<LinkedLocation, Problem> {
	let location = inspect(repository, participant)?;
	if location.competing {
		return Err(super::portability::placement_problem(
			&location.registered,
			&location.destination,
			true,
		));
	}
	Ok(location)
}

pub(super) fn inspect(
	repository: &Repository,
	participant: &Participant,
) -> Result<LinkedLocation, Problem> {
	let identity = participant.worktree_identity.as_ref();
	let entries = std::fs::read_dir(repository.common_dir.join("worktrees"))
		.map_err(|error| {
			if error.kind() == std::io::ErrorKind::NotFound {
				return changed("The worktree administrative identity changed or no longer exists in this repository. Restore the missing worktree; recovery will not recreate authored work.");
			}
			Problem::io("Inspect linked-worktree administration", error)
		})?;
	for entry in entries {
		let entry = entry.map_err(|error| Problem::io("Read linked-worktree entry", error))?;
		let administration = entry.path();
		let observed_identity = workyard_system::directory_identity(&administration)?;
		if identity.is_some_and(|expected| expected != &observed_identity) {
			continue;
		}
		let backlink = read_path(&administration.join("gitdir"))?;
		let current = worktree_from_backlink(&administration, &backlink)?;
		if identity.is_none() && current != participant.destination {
			continue;
		}
		let registration = Registration::inspect(current, &administration, &backlink, participant)?;
		return resolve(registration, &administration, participant);
	}
	Err(changed(
		"The worktree administrative identity changed or no longer exists in this repository.",
	))
}

fn worktree_from_backlink(admin: &Path, backlink: &Path) -> Result<PathBuf, Problem> {
	let mut tree = absolute(admin, backlink);
	if !tree.pop() {
		return Err(changed("Git worktree registration has no parent."));
	}
	Ok(tree)
}

impl Registration {
	fn inspect(
		current: PathBuf,
		admin: &Path,
		backlink: &Path,
		participant: &Participant,
	) -> Result<Self, Problem> {
		let hint = participant.placement.as_ref();
		let bound = matches_pointer(&current, admin, hint, std::fs::metadata)?;
		let registration = Self {
			tree: current,
			bound,
			current_backlink: true,
		};
		let Some(hint) = hint.filter(|_| backlink.is_relative()) else {
			return Ok(registration);
		};

		let previous_admin = hint
			.common_dir
			.join("worktrees")
			.join(admin.file_name().expect("administrative entry"));
		let previous = worktree_from_backlink(&previous_admin, backlink)?;
		if same_path(&previous, &registration.tree) {
			return Ok(registration);
		}

		// When both ends resolve from their current locations Git already has
		// a valid registration. Otherwise the clone's move may have changed
		// the meaning of the relative backlink while its original tree still lives.
		if bound && matches_pointer(&registration.tree, admin, None, std::fs::metadata)? {
			return Ok(registration);
		}
		let previous_bound = matches_pointer(&previous, admin, Some(hint), std::fs::metadata)?;
		if previous_bound || (!bound && same_path(&previous, &hint.destination)) {
			return Ok(Self {
				tree: previous,
				bound: previous_bound,
				current_backlink: false,
			});
		}
		Ok(registration)
	}
}

fn resolve(
	registration: Registration,
	administration: &Path,
	participant: &Participant,
) -> Result<LinkedLocation, Problem> {
	let registered = registration.tree.as_path();
	let bound = registration.bound;
	let expected = &participant.destination;
	let local = if matches_pointer(
		expected,
		administration,
		participant.placement.as_ref(),
		std::fs::symlink_metadata,
	)? {
		Some(expected.clone())
	} else if participant.worktree_identity.is_some() {
		discover_task_child(administration, participant)?
	} else {
		None
	};

	let destination = match local {
		Some(path) => {
			let physical = path
				.canonicalize()
				.map_err(|e| Problem::io("Resolve local worktree", e))?;
			if !same_path(&path, &physical) {
				return Err(changed(
					"The local worktree path crosses a directory alias. Restore a direct location before recovery.",
				));
			}
			physical
		}
		None if bound => {
			verify_external(registered, expected, participant)?;
			registered.to_owned()
		}
		None => {
			return Err(changed(
				"The participant worktree is missing. Restore it or supply its actual location through Git repair before retrying.",
			));
		}
	};
	let destination = destination
		.canonicalize()
		.map_err(|e| Problem::io("Resolve linked worktree location", e))?;
	let same_tree = same_path(&destination, registered);
	if !bound && !same_tree {
		verify_previous_volume(registered, participant)?;
	}
	Ok(LinkedLocation {
		administration: administration.to_owned(),
		registered: registered.to_owned(),
		competing: bound && !same_tree,
		registration_matches: registration.current_backlink && same_tree,
		destination,
	})
}

/// A Git move preserves the working directory entry. A registration activated
/// in a different copy points to different files, even under the same admin ID.
fn verify_external(
	registered: &Path,
	expected: &Path,
	participant: &Participant,
) -> Result<(), Problem> {
	let hint = participant
		.placement
		.as_ref()
		.ok_or_else(|| changed("The external worktree has no verified location."))?;
	let local_displaced = hint.destination.starts_with(&hint.workspace)
		&& !same_path(expected, &hint.destination)
		&& registered
			.canonicalize()
			.map_err(|error| Problem::io("Resolve registered worktree", error))?
			.ancestors()
			.any(|ancestor| same_path(ancestor, &hint.workspace));
	let entry = workyard_system::entry_identity(registered)?;
	if !local_displaced && Some(entry) == hint.worktree_entry {
		return Ok(());
	}

	let exists = expected
		.try_exists()
		.map_err(|error| Problem::io("Inspect local worktree", error))?;
	let message = if exists {
		"Local participant files belong to an unverified Git registration. Restore or track the original backing repository."
	} else {
		"Local participant files are missing. The registered worktree belongs to another location."
	};
	Err(changed(message))
}

/// A missing worktree beneath an unmounted volume looks like a move until the
/// nearest reachable ancestor is checked against the last verified volume.
fn verify_previous_volume(registered: &Path, participant: &Participant) -> Result<(), Problem> {
	let Some(volume) = participant
		.placement
		.as_ref()
		.and_then(|hint| hint.worktree_entry.map(|(volume, _)| volume))
	else {
		return Err(changed(
			"Previous worktree volume is unknown; restore access or verify the registration through Git.",
		));
	};
	for ancestor in registered.ancestors() {
		if ancestor
			.try_exists()
			.map_err(|e| Problem::io("Inspect previous worktree location", e))?
		{
			if workyard_system::entry_identity(ancestor)?.0 == volume {
				return Ok(());
			}
			break;
		}
	}
	Err(changed(
		"The previous worktree volume is unavailable. Restore access and retry recovery.",
	))
}

/// Search only immediate task children, using their linked administrative pointer.
fn discover_task_child(
	administration: &Path,
	participant: &Participant,
) -> Result<Option<PathBuf>, Problem> {
	let parent = participant
		.destination
		.parent()
		.ok_or_else(|| changed("Participant has no parent directory."))?;
	if !parent
		.try_exists()
		.map_err(|error| Problem::io("Inspect task directory", error))?
	{
		return Ok(None);
	}
	let mut matches = Vec::new();
	for entry in
		std::fs::read_dir(parent).map_err(|error| Problem::io("Read task directory", error))?
	{
		let entry = entry.map_err(|error| Problem::io("Read task child", error))?;
		if !entry
			.file_type()
			.map_err(|error| Problem::io("Inspect task child", error))?
			.is_dir()
		{
			continue;
		}
		if matches_pointer(
			&entry.path(),
			administration,
			participant.placement.as_ref(),
			std::fs::symlink_metadata,
		)? {
			matches.push(entry.path());
		}
	}
	if matches.len() > 1 {
		return Err(changed(
			"Several task children refer to this worktree identity. Preserve all files and explicitly repair Git at the intended location.",
		));
	}
	Ok(matches.pop())
}
