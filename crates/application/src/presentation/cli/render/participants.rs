//! Recorded membership and current Git facts use distinct participant rows.
use crate::observation::{ParticipantState, TaskReport};
use crate::presentation::cli::render::{LocatedParticipantRow, ParticipantRow, Terminal, Tone};
use std::path::Path;

pub(super) fn inspected(
	terminal: &mut Terminal,
	task: &TaskReport,
	root: Option<&Path>,
	recovery_state: Option<&Path>,
	detail: bool,
) {
	let rows = task
		.current
		.iter()
		.map(|participant| {
			let tone = participant_tone(participant.state, participant.operation_in_progress);
			let state = if participant.operation_in_progress == Some(true) {
				format!("{} · Git operation unfinished", participant.state.label())
			} else {
				participant.state.label().into()
			};

			LocatedParticipantRow {
				participant: ParticipantRow {
					name: participant.name.clone(),
					state,
					tone,
				},
				location: participant.current_path.as_ref().map_or_else(
					|| "unavailable".into(),
					|path| display_path(terminal, root, path.as_path()),
				),
				backing: participant.backing_repo.as_ref().map_or_else(
					|| "unavailable".into(),
					|path| display_path(terminal, root, path.as_path()),
				),
			}
		})
		.collect::<Vec<_>>();
	terminal.located_participants(&rows);

	let recover = task
		.name
		.as_deref()
		.filter(|_| !task.busy && task.pending_participant.is_none())
		.and_then(|name| crate::guidance::commands::recover(name, recovery_state));
	let mut suggest_recovery = false;
	for participant in &task.current {
		if detail
			&& let Some(branch) = &participant.branch
			&& task.name.as_deref() != Some(branch.as_str())
		{
			terminal.note(&format!("{} branch: {branch}", participant.name));
		}
		if let Some(problem) = &participant.problem {
			if participant.state == ParticipantState::Missing
				&& problem.code == "repository_changed"
				&& recover.is_some()
			{
				suggest_recovery = true;
				continue;
			}
			terminal.participant_problem(&participant.name, problem);
		}
	}
	if suggest_recovery && let Some(command) = recover {
		terminal.hint(&format!("Try: {command}"));
	}
}

pub(super) fn recorded(terminal: &mut Terminal, task: &TaskReport, root: Option<&Path>) {
	let Some(saved) = task.recorded.as_ref().or(task.evidence.as_ref()) else {
		return;
	};

	let rows = saved
		.participants
		.iter()
		.map(|participant| {
			let state = if participant.phase == "completed" {
				"registered"
			} else {
				"incomplete"
			};

			LocatedParticipantRow {
				participant: ParticipantRow {
					name: participant.name.clone(),
					state: state.into(),
					tone: Tone::Neutral,
				},
				location: display_path(terminal, root, participant.path.as_path()),
				backing: saved
					.backing_repositories
					.get(&participant.name)
					.map_or_else(
						|| "unavailable".into(),
						|path| display_path(terminal, root, path.as_path()),
					),
			}
		})
		.collect::<Vec<_>>();
	terminal.located_participants(&rows);
}

fn display_path(terminal: &Terminal, root: Option<&Path>, path: &Path) -> String {
	root.map_or_else(
		|| terminal.paths.root(path),
		|root| terminal.paths.within(root, path),
	)
}

fn participant_tone(state: ParticipantState, unfinished: Option<bool>) -> Tone {
	match state {
		ParticipantState::Conflicted
		| ParticipantState::Missing
		| ParticipantState::Unavailable => Tone::Attention,
		ParticipantState::Clean if unfinished != Some(true) => Tone::Good,
		_ => Tone::Neutral,
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn unfinished_operation_does_not_downgrade_conflict_tone() {
		assert!(matches!(
			participant_tone(ParticipantState::Conflicted, Some(true)),
			Tone::Attention
		));
		assert!(matches!(
			participant_tone(ParticipantState::Clean, Some(true)),
			Tone::Neutral
		));
	}
}
