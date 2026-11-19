//! Membership changes shared by amendment and named recovery results.
use super::{MessageScope, Terminal, text};
use crate::contracts::{ParticipantChange, ParticipantOutcome};

impl Terminal {
	pub fn removed_participants(
		&mut self,
		changes: &[ParticipantChange],
		members: &[ParticipantOutcome],
	) {
		let (replaced, removed): (Vec<_>, Vec<_>) = changes
			.iter()
			.filter(|change| change.kind == "removed")
			.partition(|change| {
				members
					.iter()
					.any(|member| member.name == change.participant)
			});

		for (label, changes) in [("Removed", removed), ("Replaced", replaced)] {
			if changes.is_empty() {
				continue;
			}
			let names = changes
				.iter()
				.map(|change| text(&change.participant))
				.collect::<Vec<_>>()
				.join(", ");
			self.hint_at(MessageScope::Workspace, &format!("{label}: {names}"));
		}
	}
}
