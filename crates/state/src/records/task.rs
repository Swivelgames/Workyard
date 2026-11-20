mod codec;
mod compatibility;
mod operation;
mod placement;
mod removal;
pub use removal::RemovalRecord;

pub(crate) use codec::{decode, match_intent, validate};
pub use operation::OperationRecord;

use crate::path::StoredPath;
use serde::{Deserialize, Serialize};
use workyard_core::{Problem, Task};

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct CreationRecord {
	pub checkout: bool,
	pub no_fetch: bool,
	#[serde(rename = "pull")]
	legacy_pull: bool,
}

impl From<workyard_core::CreationOptions> for CreationRecord {
	fn from(value: workyard_core::CreationOptions) -> Self {
		Self {
			checkout: value.checkout,
			no_fetch: value.no_fetch,
			legacy_pull: false,
		}
	}
}

impl From<CreationRecord> for workyard_core::CreationOptions {
	fn from(value: CreationRecord) -> Self {
		Self {
			checkout: value.checkout,
			no_fetch: value.no_fetch,
		}
	}
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ParticipantRecord {
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub last_location: Option<placement::LocationRecord>,
	#[serde(default, skip_serializing_if = "Vec::is_empty")]
	pub registration_roots: Vec<StoredPath>,
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub creation: Option<CreationRecord>,
	pub name: String,
	pub source: StoredPath,
	pub common_dir: StoredPath,
	pub backing_identity: String,
	#[serde(default)]
	pub worktree_identity: Option<String>,
	pub destination: StoredPath,
	pub remote: Option<String>,
	pub from: Option<String>,
	pub phase: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ProblemRecord {
	pub code: String,
	pub message: String,
	pub remedy: String,
}

impl From<&Problem> for ProblemRecord {
	fn from(problem: &Problem) -> Self {
		Self {
			code: problem.code.as_str().into(),
			message: problem.message.clone(),
			remedy: problem.remedy.clone(),
		}
	}
}

#[derive(Clone, Debug, Serialize)]
pub struct TaskRecord {
	pub generation: u64,
	pub removal: Option<RemovalRecord>,
	pub version: u32,
	pub task: String,
	pub tasks_root: StoredPath,
	pub state_dir: StoredPath,
	pub checkout: bool,
	pub no_fetch: bool,
	#[serde(rename = "pull")]
	legacy_pull: bool,
	pub lifecycle: String,
	pub operation: OperationRecord,
	pub participants: Vec<ParticipantRecord>,
	pub problem: Option<ProblemRecord>,
}

impl From<&Task> for TaskRecord {
	fn from(task: &Task) -> Self {
		Self {
			version: if task.participants.iter().any(|p| p.creation.is_some())
				|| task.operation.kind == Some(workyard_core::OperationKind::Amend)
				|| task.operation.continues == Some(workyard_core::OperationKind::Amend)
			{
				4
			} else {
				3
			},
			generation: task.generation,
			removal: task.removal.as_ref().map(Into::into),
			task: task.name.clone(),
			tasks_root: placement::anchored(&task.root, &task.state_dir),
			state_dir: task.state_dir.as_path().into(),
			checkout: task.checkout,
			no_fetch: task.no_fetch,
			legacy_pull: false,
			lifecycle: task.lifecycle.as_str().into(),
			operation: OperationRecord::from(task.operation),
			problem: task.problem.as_ref().map(Into::into),
			participants: task
				.participants
				.iter()
				.map(|p| ParticipantRecord::new(p, &task.state_dir))
				.collect(),
		}
	}
}

impl ParticipantRecord {
	fn new(participant: &workyard_core::Participant, state: &std::path::Path) -> Self {
		Self {
			last_location: placement::LocationRecord::from_participant(participant, state),
			creation: participant.creation.map(Into::into),
			name: participant.name.clone(),
			source: placement::anchored(&participant.source, state),
			common_dir: placement::anchored(&participant.common_dir, state),
			backing_identity: participant.backing_identity.clone(),
			registration_roots: participant
				.registration_roots
				.iter()
				.map(|p| p.as_path().into())
				.collect(),
			worktree_identity: participant.worktree_identity.clone(),
			destination: placement::anchored(&participant.destination, state),
			remote: participant.remote.clone(),
			from: participant.from.clone(),
			phase: participant.phase.as_str().into(),
		}
	}
}

impl TaskRecord {
	const SUPPORTED_VERSIONS: &[u64] = &[1, 2, 3, 4];

	pub(crate) fn read(path: &std::path::Path) -> Result<Self, Problem> {
		Self::read_with_version(path).map(|(record, _)| record)
	}

	/// Legacy records normalize on decode. Recovery also needs their original on-disk version.
	pub(crate) fn read_with_version(path: &std::path::Path) -> Result<(Self, u64), Problem> {
		crate::documents::read_supported_with_version(path, Self::SUPPORTED_VERSIONS)
	}

	pub(crate) fn refresh_location(
		&mut self,
		participant: &workyard_core::Participant,
		state: &std::path::Path,
	) -> Result<bool, Problem> {
		let record = self
			.participants
			.iter_mut()
			.find(|record| {
				record.name == participant.name
					&& record.backing_identity == participant.backing_identity
					&& record.worktree_identity == participant.worktree_identity
			})
			.ok_or_else(|| {
				crate::damage::damaged("Participant identity changed during placement repair.")
			})?;
		let current = ParticipantRecord::new(participant, state);
		let changed = record.source != current.source
			|| record.common_dir != current.common_dir
			|| record.destination != current.destination
			|| record.last_location != current.last_location;
		if changed {
			record.source = current.source;
			record.common_dir = current.common_dir;
			record.destination = current.destination;
			record.last_location = current.last_location;
		}
		Ok(changed)
	}
}
