use super::super::workspace::{PlacementState, WorkspaceRecovery};
use crate::presentation::cli::render::{MessageScope, Terminal, text};
use std::io;
use trine::application::CliViewContext;

pub(super) fn finish(output: &CliViewContext, outcome: &WorkspaceRecovery) -> io::Result<()> {
	let mut terminal = Terminal::for_output(&output.stdout);
	if outcome.nothing_to_recover {
		terminal.heading("Nothing to recover");
	}
	let mut previous = None;
	for row in &outcome.participants {
		if row.state == PlacementState::Current {
			continue;
		}
		if previous != row.task.as_deref() {
			previous = row.task.as_deref();
			terminal.heading(previous.unwrap_or("Unknown task"));
		}
		let name = text(row.participant.as_deref().unwrap_or("Task"));
		if let Some(problem) = &row.problem {
			terminal.participant_problem(&name, problem);
		} else if let (Some(old), Some(new)) = (&row.registered, &row.destination) {
			let old = terminal.paths.root(old.as_path());
			let new = terminal.paths.root(new.as_path());
			let state = if row.state == PlacementState::Repaired {
				"repaired"
			} else {
				"proposed"
			};
			terminal.note(&format!("{name} [{state}]  {old} -> {new}"));
		}
	}
	if let Some(problem) = &outcome.problem {
		terminal.problem_at(MessageScope::Workspace, problem);
	}
	if terminal.output.is_empty() {
		terminal.heading("Workspace location updated");
	}
	output.stdout.write_all(terminal.output.as_bytes())
}
