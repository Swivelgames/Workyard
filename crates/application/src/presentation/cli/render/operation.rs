use super::{MessageScope, ParticipantRow, TaskTag, Terminal, Tone};
use crate::contracts::ParticipantOutcome;
use std::path::Path;

impl Terminal {
	pub fn status_guidance(&mut self, task: &str, workspace: Option<&Path>) {
		if let Some(command) = crate::guidance::commands::status(task, workspace) {
			self.hint_at(MessageScope::Section, &format!("Inspect: {command}"));
		} else {
			let hint = if workspace.is_some() {
				"Inspect with workyard status in the selected workspace."
			} else {
				"Inspect the task with workyard status."
			};
			self.hint_at(MessageScope::Section, hint);
		}
	}

	pub fn operation_participants(
		&mut self,
		participants: &[ParticipantOutcome],
		root: Option<&Path>,
		task: &str,
		completed_label: &str,
	) {
		if participants.is_empty() {
			return;
		}
		let rows = participants
			.iter()
			.map(|participant| {
				let (state, tone) = match participant.phase.as_str() {
					"completed" => (completed_label, Tone::Neutral),
					"present" => ("present", Tone::Neutral),
					"removed" => ("removed", Tone::Neutral),
					"restored" => ("restored", Tone::Neutral),
					"restoration_unconfirmed" => ("restoration unconfirmed", Tone::Neutral),
					"worktree_created" => ("worktree created (recorded)", Tone::Neutral),
					"not_started" => ("not started", Tone::Neutral),
					"fetching" => ("fetch unconfirmed", Tone::Neutral),
					"creating" => ("creation unconfirmed", Tone::Neutral),
					_ => TaskTag::NeedsAttention.appearance(),
				};

				ParticipantRow {
					name: participant.name.clone(),
					state: state.into(),
					tone,
				}
			})
			.collect::<Vec<_>>();
		self.participants(&rows);

		if let Some(root) = root {
			for participant in participants {
				if participant.path.as_path() != root.join(task).join(&participant.name) {
					self.note(&format!(
						"{} location (relative to root): {}",
						participant.name,
						self.paths.within(root, participant.path.as_path())
					));
				}
			}
		}
	}
}
