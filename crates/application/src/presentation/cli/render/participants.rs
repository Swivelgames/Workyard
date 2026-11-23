//! Recorded membership and current Git facts use distinct participant rows.
use crate::observation::{ParticipantReport, ParticipantState, SavedTask, TaskReport};
use crate::presentation::cli::render::{ParticipantRow, StatusParticipantRow, Terminal, Tone};
use std::path::Path;
use workyard_core::ProblemDetail;

pub(super) fn inspected(
	terminal: &mut Terminal,
	task: &TaskReport,
	workspace: Option<&Path>,
	tasks_root: Option<&Path>,
	explicit_workspace: Option<&Path>,
) {
	let saved = task.recorded.as_ref().or(task.evidence.as_ref());
	let settled = !task.busy && task.pending_participant.is_none();
	let rows: Vec<_> = task
		.current
		.iter()
		.map(|participant| inspected_row(terminal, participant, workspace, saved, settled))
		.collect();
	terminal.status_participants(&rows);

	let task_dir = tasks_root
		.zip(task.name.as_deref())
		.map(|(root, name)| root.join(name));
	let mut repairable = false;
	for participant in &task.current {
		let expected = task_dir.as_ref().map(|dir| dir.join(&participant.name));
		location_notes(
			terminal,
			participant,
			workspace,
			expected.as_deref(),
			task.name.as_deref(),
		);
		if !recorded_removed(participant, saved.filter(|_| settled)) {
			repairable |= render_problem(terminal, participant, workspace);
		}
	}

	if repairable
		&& settled
		&& let Some(command) = task
			.name
			.as_deref()
			.and_then(|name| crate::guidance::commands::recover(name, explicit_workspace))
	{
		terminal.hint(&format!("Try: {command}"));
	}
}

fn inspected_row(
	terminal: &Terminal,
	participant: &ParticipantReport,
	workspace: Option<&Path>,
	saved: Option<&SavedTask>,
	settled: bool,
) -> StatusParticipantRow {
	let absence = recorded_absence(participant, saved.filter(|_| settled));
	let (state, tone) = if let Some(label) = absence {
		(label.into(), Tone::Neutral)
	} else {
		let label = participant.state.label();
		let unfinished = participant.operation_in_progress;
		let state = if unfinished == Some(true) {
			format!("{label} · Git operation unfinished")
		} else {
			label.into()
		};
		(state, participant_tone(participant.state, unfinished))
	};

	let backing = participant
		.repository_source
		.as_ref()
		.or(participant.backing_repo.as_ref())
		.or_else(|| saved.and_then(|saved| saved.repository_sources.get(&participant.name)));

	StatusParticipantRow {
		participant: ParticipantRow {
			name: participant.name.clone(),
			state,
			tone,
		},
		backing: display_repository(terminal, workspace, backing.map(|path| path.as_path())),
	}
}

fn recorded_removed(participant: &ParticipantReport, saved: Option<&SavedTask>) -> bool {
	participant.state == ParticipantState::Missing
		&& saved.is_some_and(|saved| saved.removed.contains(&participant.name))
}

fn recorded_absence(
	participant: &ParticipantReport,
	saved: Option<&SavedTask>,
) -> Option<&'static str> {
	if participant.state != ParticipantState::Missing {
		return None;
	}
	if recorded_removed(participant, saved) {
		return Some("removed");
	}
	let member = saved?
		.participants
		.iter()
		.find(|member| member.name == participant.name)?;
	matches!(member.phase.as_str(), "not_started" | "fetching").then_some("not created")
}

fn location_notes(
	terminal: &mut Terminal,
	participant: &ParticipantReport,
	workspace: Option<&Path>,
	expected: Option<&Path>,
	task: Option<&str>,
) {
	let name = &participant.name;
	let expected = expected.unwrap_or(participant.recorded_path.as_path());
	if let Some(path) = participant.current_path.as_ref().map(|path| path.as_path())
		&& path != expected
	{
		let location = display_path(terminal, workspace, path);
		terminal.note(&format!("{name} is at {location}"));
	}
	if let Some(branch) = &participant.branch
		&& task != Some(branch.as_str())
	{
		terminal.note(&format!("{name} branch: {branch}"));
	}
}

/// Returns whether the observed placement supports a named recovery hint.
fn render_problem(
	terminal: &mut Terminal,
	participant: &ParticipantReport,
	workspace: Option<&Path>,
) -> bool {
	let Some(problem) = &participant.problem else {
		return false;
	};
	let name = &participant.name;
	if let Some(ProblemDetail::MissingWorktree {
		expected, occupied, ..
	}) = problem.detail.as_deref()
	{
		let path = display_path(terminal, workspace, expected);
		let message = if *occupied {
			format!(
				"{name}: {path} is occupied. Keep the files there and locate the original worktree."
			)
		} else {
			format!("{name}: restore {path}, or repair its Git link if it moved.")
		};
		terminal.note(&message);
		return false;
	}

	if problem.code == "repository_changed" {
		if let Some(placement) = &problem.worktree_locations {
			let location = display_path(terminal, workspace, placement.candidate.as_path());
			let message = if placement.registered.as_path() == placement.candidate.as_path() {
				format!("{name}: Git link needs repair at {location}.")
			} else {
				format!("{name} moved to {location}. Git link needs repair.")
			};
			terminal.note(&message);
			return true;
		}
		if participant.state == ParticipantState::Missing {
			terminal.note(&format!(
				"{name}: Restore the worktree or repair its Git link if it moved."
			));
			return false;
		}
	}

	terminal.participant_problem(name, problem);
	false
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

			StatusParticipantRow {
				participant: ParticipantRow {
					name: participant.name.clone(),
					state: state.into(),
					tone: Tone::Neutral,
				},
				backing: display_repository(
					terminal,
					root,
					saved
						.repository_sources
						.get(&participant.name)
						.map(|path| path.as_path()),
				),
			}
		})
		.collect::<Vec<_>>();
	terminal.status_participants(&rows);
}

fn display_repository(
	terminal: &Terminal,
	workspace: Option<&Path>,
	backing: Option<&Path>,
) -> String {
	let Some(backing) = backing else {
		return "unavailable".into();
	};
	display_path(terminal, workspace, backing)
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
