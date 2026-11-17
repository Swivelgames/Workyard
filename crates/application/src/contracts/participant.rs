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
