//! Small fact representations shared by action contracts.
use serde::{Serialize, Serializer};
use trine::NativePath;
use workyard_core::{Participant, TaskStatus};

#[derive(Serialize)]
pub struct ParticipantOutcome {
	pub name: String,
	pub path: NativePath,
	pub phase: String,
}

impl From<Participant> for ParticipantOutcome {
	fn from(participant: Participant) -> Self {
		Self {
			name: participant.name,
			path: NativePath::new(participant.destination).expect("validated filesystem path"),
			phase: participant.phase.as_str().into(),
		}
	}
}

pub(crate) fn serialize_status<S: Serializer>(
	status: &TaskStatus,
	serializer: S,
) -> Result<S::Ok, S::Error> {
	serializer.serialize_str(status.as_str())
}
