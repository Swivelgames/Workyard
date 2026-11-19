//! Atomic membership authority anchored to the original task admission.
mod request;
mod targets;
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
pub(crate) use targets::TargetRecord;
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
	#[serde(default, skip_serializing_if = "Vec::is_empty")]
	pub removed: Vec<TargetRecord>,
	pub request: Option<RequestRecord>,
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
	if let Some(request) = &revision.request {
		request.validate(&intent.task, state)?;
	} else if revision.current.task.removal.is_none() {
		return Err(damaged(
			"Membership revision has no owning amendment or retirement.",
		));
	}
	if revision.current.task.generation != intent.generation
		|| revision
			.current
			.pending
			.is_some_and(|index| index >= revision.current.task.participants.len())
	{
		return Err(damaged(
			"Membership revision disagrees with its task incarnation.",
		));
	}
	targets::validate(&revision)?;
	Ok(Some(revision))
}
