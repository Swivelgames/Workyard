use crate::damage::damaged;
use serde::{Deserialize, Serialize};
use workyard_core::{Problem, Removal, RemovalDirection, RemovalParticipant, RemovalPhase};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RemovalParticipantRecord {
	pub branch: Option<String>,
	pub dirty_at_start: bool,
	pub phase: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RemovalRecord {
	pub direction: String,
	pub participants: Vec<RemovalParticipantRecord>,
	pub retired_at: Option<u64>,
}

impl From<&Removal> for RemovalRecord {
	fn from(removal: &Removal) -> Self {
		Self {
			direction: match removal.direction {
				RemovalDirection::Remove => "remove",
				RemovalDirection::Restore => "restore",
			}
			.into(),
			retired_at: removal.retired_at,
			participants: removal
				.participants
				.iter()
				.map(|participant| RemovalParticipantRecord {
					branch: participant.branch.clone(),
					dirty_at_start: participant.dirty_at_start,
					phase: match participant.phase {
						RemovalPhase::Present => "present",
						RemovalPhase::Removed => "removed",
						RemovalPhase::Restoring => "restoring",
						RemovalPhase::Recreated => "recreated",
						RemovalPhase::Restored => "restored",
					}
					.into(),
				})
				.collect(),
		}
	}
}

impl RemovalRecord {
	pub(super) fn decode(&self) -> Result<Removal, Problem> {
		let direction = match self.direction.as_str() {
			"remove" => RemovalDirection::Remove,
			"restore" => RemovalDirection::Restore,
			_ => return Err(damaged("Invalid removal direction.")),
		};
		let participants = self
			.participants
			.iter()
			.map(|participant| {
				let phase = match participant.phase.as_str() {
					"present" => RemovalPhase::Present,
					"removed" => RemovalPhase::Removed,
					"restoring" => RemovalPhase::Restoring,
					"recreated" => RemovalPhase::Recreated,
					"restored" => RemovalPhase::Restored,
					_ => return Err(damaged("Invalid removal participant phase.")),
				};
				Ok(RemovalParticipant {
					branch: participant.branch.clone(),
					dirty_at_start: participant.dirty_at_start,
					phase,
				})
			})
			.collect::<Result<_, Problem>>()?;

		Ok(Removal {
			direction,
			participants,
			retired_at: self.retired_at,
		})
	}
}
