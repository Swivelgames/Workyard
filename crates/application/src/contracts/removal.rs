use serde::Serialize;
use trine::NativePath;

use super::{Condition, ParticipantOutcome};
use workyard_core::{Participant, Removal, RemovalPhase};

#[derive(Clone, Serialize)]
pub struct Disclosure {
	pub participant: String,
	pub dirty_paths: Vec<NativePath>,
	pub history: &'static str,
}

#[derive(Clone, Serialize)]
pub struct DecisionRequirement {
	/// Opaque approval for the current disclosure.
	#[serde(skip_serializing_if = "Option::is_none")]
	pub token: Option<String>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub task: Option<String>,
	pub kind: String,
	#[serde(skip_serializing_if = "Vec::is_empty")]
	pub participants: Vec<String>,
	pub participant: String,
	pub message: String,
}

impl From<workyard_core::Decision> for DecisionRequirement {
	fn from(decision: workyard_core::Decision) -> Self {
		use workyard_core::DecisionKind;
		let kind = match decision.kind {
			DecisionKind::RemoveParticipants => "remove_participants",
			DecisionKind::DiscardParticipants => "discard_participants",
			DecisionKind::RemoveTask => "remove_task",
			DecisionKind::DiscardTask => "discard_task",
			DecisionKind::DiscardFiles => "discard_files",
			DecisionKind::RetainedHistory => "retained_history",
			DecisionKind::LostChanges => "lost_changes",
		};
		Self {
			token: None,
			kind: kind.into(),
			task: decision.task,
			participant: decision.participant,
			participants: decision.participants,
			message: decision.message,
		}
	}
}

#[derive(Default, Serialize)]
pub struct RemovalReport {
	#[serde(skip_serializing_if = "Vec::is_empty")]
	pub disclosures: Vec<Disclosure>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub decision: Option<DecisionRequirement>,
}

/// Cancellation is an invocation outcome; task condition comes from known facts.
pub(crate) fn condition(result: &workyard_core::RemovalResult) -> Condition {
	if result.mutation_started {
		return if result.operation.outcome == workyard_core::OperationOutcome::Succeeded {
			Condition::Healthy
		} else {
			Condition::NeedsAttention
		};
	}

	if result.needs_attention {
		return Condition::NeedsAttention;
	}

	if result.inspected && result.record_available {
		return Condition::Healthy;
	}

	Condition::Unknown
}

pub(crate) fn participants(
	members: Vec<Participant>,
	removal: Option<&Removal>,
) -> Vec<ParticipantOutcome> {
	let mut participants: Vec<_> = members.into_iter().map(ParticipantOutcome::from).collect();
	if let Some(removal) = removal {
		for (participant, progress) in participants.iter_mut().zip(&removal.participants) {
			participant.phase = match progress.phase {
				RemovalPhase::Present => "present",
				RemovalPhase::Removed => "removed",
				RemovalPhase::Restoring | RemovalPhase::Recreated => "restoration_unconfirmed",
				RemovalPhase::Restored => "restored",
			}
			.into();
		}
	}
	participants
}

pub(crate) fn mark_present_after_inspection(participants: &mut [ParticipantOutcome]) {
	for participant in participants {
		if participant.phase == "completed" {
			participant.phase = "present".into();
		}
	}
}
