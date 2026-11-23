//! Membership changes reported by amendment and amendment recovery.
use super::{MessageScope, Terminal, separate, text};
use crate::contracts::{ParticipantChange, ParticipantOutcome};

impl Terminal {
	pub fn participant_changes(
		&mut self,
		changes: &[ParticipantChange],
		members: &[ParticipantOutcome],
	) {
		let groups = [
			"Added", "Renamed", "Repaired", "Updated", "Removed", "Replaced",
		];
		let mut wrote_changes = false;

		for label in groups {
			let names = changes
				.iter()
				.filter(|change| change_label(change, members) == Some(label))
				.map(|change| text(&change.participant))
				.collect::<Vec<_>>()
				.join(", ");
			if names.is_empty() {
				continue;
			}

			if !wrote_changes {
				separate(self);
				wrote_changes = true;
			}
			self.note_at(MessageScope::Section, &format!("{label}: {names}"));
		}
	}
}

fn change_label(
	change: &ParticipantChange,
	members: &[ParticipantOutcome],
) -> Option<&'static str> {
	Some(match change.kind {
		"added" => "Added",
		"renamed" => "Renamed",
		"repaired" => "Repaired",
		"updated" => "Updated",
		"removed"
			if members
				.iter()
				.any(|member| member.name == change.participant) =>
		{
			"Replaced"
		}
		"removed" => "Removed",
		_ => return None,
	})
}
