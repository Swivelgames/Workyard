use super::{Provenance, TaskReport};
use workyard_core::OperationOutcome;

use crate::contracts::Condition;

impl TaskReport {
	pub(super) fn classify(&mut self) {
		if self.lifecycle == Some("retired")
			&& self.problems.is_empty()
			&& self.pending_participant.is_none()
		{
			self.condition = Condition::Healthy;
			return;
		}
		self.condition = if self.needs_attention() {
			Condition::NeedsAttention
		} else if self.is_uncertain() {
			Condition::Unknown
		} else {
			Condition::Healthy
		};
	}

	fn needs_attention(&self) -> bool {
		let saved = self.recorded.as_ref().or(self.evidence.as_ref());
		let incomplete = saved.is_some_and(|task| {
			task.participants
				.iter()
				.any(|participant| participant.phase != "completed")
				|| (task.operation.outcome != OperationOutcome::Succeeded
					&& task.operation.kind != Some("amend")
					&& task.operation.continues != Some("amend"))
		});
		!self.problems.is_empty()
			|| self.recorded.is_none()
			|| incomplete
			|| self.current.iter().any(|participant| {
				participant.problem.is_some()
					|| matches!(
						participant.state,
						super::ParticipantState::Missing
							| super::ParticipantState::Conflicted
							| super::ParticipantState::Unavailable
					)
			})
	}

	fn is_uncertain(&self) -> bool {
		self.provenance == Provenance::Recorded
			|| self.busy
			|| self.pending_participant.is_some()
			|| self.current.is_empty()
			|| self
				.current
				.iter()
				.any(|participant| participant.operation_in_progress == Some(true))
	}
}
