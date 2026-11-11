//! Recorded membership and current Git facts use distinct participant rows.
use crate::observation::{ParticipantState, TaskReport};
use crate::terminal::{ParticipantRow, Terminal, Tone};
use std::path::Path;

pub(super) fn inspected(
	terminal: &mut Terminal,
	task: &TaskReport,
	root: Option<&Path>,
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

			ParticipantRow {
				name: participant.name.clone(),
				state,
				tone,
			}
		})
		.collect::<Vec<_>>();
	terminal.participants(&rows);

	for participant in &task.current {
		if let Some(path) = &participant.current_path {
			location(
				terminal,
				root,
				task.name.as_deref(),
				&participant.name,
				path.as_path(),
			);
		}
		if detail
			&& let Some(branch) = &participant.branch
			&& task.name.as_deref() != Some(branch.as_str())
		{
			terminal.note(&format!("{} branch: {branch}", participant.name));
		}
		if let Some(problem) = &participant.problem {
			terminal.participant_problem(&participant.name, problem);
		}
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

			ParticipantRow {
				name: participant.name.clone(),
				state: state.into(),
				tone: Tone::Neutral,
			}
		})
		.collect::<Vec<_>>();
	terminal.participants(&rows);

	for participant in &saved.participants {
		location(
			terminal,
			root,
			task.name.as_deref(),
			&participant.name,
			participant.path.as_path(),
		);
	}
}

fn location(
	terminal: &mut Terminal,
	root: Option<&Path>,
	task: Option<&str>,
	participant: &str,
	path: &Path,
) {
	let Some(root) = root else {
		return;
	};
	if task.is_some_and(|task| root.join(task).join(participant) == path) {
		return;
	}
	terminal.note(&format!(
		"{participant} location (relative to root): {}",
		terminal.paths.within(root, path)
	));
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
