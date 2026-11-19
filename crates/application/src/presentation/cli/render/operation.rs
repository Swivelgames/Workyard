use super::{ParticipantRow, Terminal, Tone};
use crate::contracts::ParticipantOutcome;
use std::path::Path;
use workyard_core::OperationOutcome;

impl Terminal {
	pub fn status_guidance(&mut self, task: &str, state: Option<&Path>) {
		if let Some(command) = crate::guidance::commands::status(task, state) {
			self.hint(&format!("Inspect: {command}"));
		} else {
			self.hint("Inspect with workyard status, supplying the task and --workspace using your shell's native-path quoting.");
		}
	}

	pub fn operation_result(&mut self, label: &str, outcome: OperationOutcome) {
		self.heading(&format!("{label} {}", outcome.as_str()));
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
				let state = match participant.phase.as_str() {
					"completed" => completed_label,
					"present" => "present",
					"removed" => "removed",
					"restored" => "restored",
					"restoration_unconfirmed" => "restoration unconfirmed",
					"worktree_created" => "worktree created (recorded)",
					"pulling" => "pull needs review",
					"not_started" | "fetching" => "creation unconfirmed",
					_ => "needs attention",
				};

				ParticipantRow {
					name: participant.name.clone(),
					state: state.into(),
					tone: Tone::Neutral,
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
