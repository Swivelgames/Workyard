//! Atomic membership authority anchored to the original task admission.
mod request;
use crate::{
	TaskRecord,
	damage::damaged,
	documents::read_versioned,
	evidence::EffectRecord,
	exists,
	records::task::{match_intent, validate},
};
pub(crate) use request::RequestRecord;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use workyard_core::Problem;

pub(crate) enum WriterUpdate {
	Preserve,
	Pending(usize),
	Settled,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Revision {
	pub version: u32,
	pub anchor: TaskRecord,
	pub current: EffectRecord,
	pub request: RequestRecord,
}

pub(crate) fn path(intent: &Path) -> PathBuf {
	intent.with_extension("revision")
}

pub(crate) fn read(
	intent_path: &Path,
	intent: &TaskRecord,
	root: &Path,
	state: &Path,
) -> Result<Option<Revision>, Problem> {
	let path = path(intent_path);
	if !exists(&path)? {
		return Ok(None);
	}
	let revision: Revision = read_versioned(&path)?;
	validate(&revision.anchor, &intent.task, root, state)?;
	match_intent(intent, &revision.anchor)?;
	validate(&revision.current.task, &intent.task, root, state)?;
	if revision.current.version != 1 {
		return Err(damaged("Unsupported revision effect format."));
	}
	let request = revision.request.decode()?;
	if request.repositories.is_empty()
		|| (request.options.no_fetch && request.options.pull)
		|| request.repositories.iter().any(|repo| {
			!repo.path.is_absolute() || repo.path.as_os_str().as_encoded_bytes().contains(&0)
		}) {
		return Err(damaged("Invalid recorded amendment request."));
	}
	if revision.current.task.generation != intent.generation
		|| revision
			.current
			.pending
			.is_some_and(|index| index >= revision.current.task.participants.len())
		|| revision.request.task != intent.task
	{
		return Err(damaged(
			"Membership revision disagrees with its task incarnation.",
		));
	}
	// Amend may add or rename membership, but cannot remove or replace its backing repositories.
	if intent.participants.iter().any(|original| {
		!revision
			.current
			.task
			.participants
			.iter()
			.any(|current| current.backing_identity == original.backing_identity)
	}) {
		return Err(damaged("Membership revision lost an admitted repository."));
	}
	Ok(Some(revision))
}
