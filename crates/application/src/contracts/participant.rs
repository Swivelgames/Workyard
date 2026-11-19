use serde::Serialize;
use trine::NativePath;
use workyard_core::Participant;

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

#[derive(Serialize)]
pub struct ParticipantChange {
	pub participant: String,
	pub kind: &'static str,
}

impl From<workyard_core::AmendmentChange> for ParticipantChange {
	fn from(change: workyard_core::AmendmentChange) -> Self {
		Self {
			participant: change.participant,
			kind: change.kind.as_str(),
		}
	}
}
