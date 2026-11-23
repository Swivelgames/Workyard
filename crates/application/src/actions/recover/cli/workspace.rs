use super::super::workspace::{PlacementReport, PlacementState, WorkspaceRecovery};
use crate::presentation::cli::render::{MessageScope, Terminal, Tone, separate, text};
use std::{fmt::Write, io};
use trine::application::CliViewContext;

pub(super) fn finish(output: &CliViewContext, outcome: &WorkspaceRecovery) -> io::Result<()> {
	let mut terminal = Terminal::for_output(&output.stdout);
	if outcome.nothing_to_recover {
		terminal.heading("No workspace location repairs needed");
		interrupted_removals(&mut terminal, outcome);
	} else if outcome.unresolved_locations {
		terminal.heading("Workspace recovery needs attention");
	}

	let mut missing_worktrees = false;
	let mut previous_task = None;
	for row in &outcome.participants {
		if row.state == PlacementState::Current {
			continue;
		}

		let task = row.task.as_deref().unwrap_or("Unknown task");
		if previous_task != Some(task) {
			previous_task = Some(task);
			separate(&mut terminal);
			if outcome.unresolved_locations {
				terminal.note_at(MessageScope::Section, task);
			} else {
				terminal.heading(task);
			}
		}
		missing_worktrees |= render_participant(&mut terminal, row);
	}
	if missing_worktrees {
		separate(&mut terminal);
		terminal.hint_at(MessageScope::Section, super::RESTORE_WORKTREE);
	}
	if !outcome.unresolved_locations
		&& let Some(problem) = &outcome.problem
	{
		separate(&mut terminal);
		terminal.problem_at(MessageScope::Workspace, problem);
	}
	if terminal.output.is_empty() {
		terminal.heading("Workspace location updated");
	}
	output.stdout.write_all(terminal.output.as_bytes())
}

/// Returns whether the workspace summary should include restoration advice.
fn render_participant(terminal: &mut Terminal, row: &PlacementReport) -> bool {
	let name = row.participant.as_deref().unwrap_or("Task");
	if let Some(problem) = &row.problem {
		if let Some(occupied) = terminal.missing_worktree(MessageScope::Task, problem) {
			return !occupied;
		}
		terminal.participant_problem(name, problem);
		return false;
	}

	if let (Some(old), Some(new)) = (&row.registered, &row.destination) {
		let old = terminal.paths.root(old.as_path());
		let new = terminal.paths.root(new.as_path());
		let tag = if row.state == PlacementState::Repaired {
			terminal.tag("repaired", Tone::Good)
		} else {
			terminal.tag("proposed", Tone::Neutral)
		};
		let name = text(name);
		writeln!(terminal.output, "      {name} {tag}  {old} -> {new}").expect("string output");
	}
	false
}

fn interrupted_removals(terminal: &mut Terminal, outcome: &WorkspaceRecovery) {
	if outcome.interrupted_removals.is_empty() {
		return;
	}
	let workspace = outcome
		.explicit_workspace
		.as_ref()
		.map(|path| path.as_path());
	separate(terminal);
	terminal.note_at(
		MessageScope::Section,
		"Continue interrupted task removals with named recovery.",
	);
	for task in &outcome.interrupted_removals {
		if let Some(command) = crate::guidance::commands::recover(task, workspace) {
			terminal.command_at(MessageScope::Section, &command);
		}
	}
}
