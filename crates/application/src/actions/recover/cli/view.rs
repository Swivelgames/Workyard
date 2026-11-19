use super::super::{RecoverOutcome, RecoveryOutcome};
use crate::{
	contracts::Progress,
	presentation::cli::render::{MessageScope, Terminal, text},
};
use std::io;
use trine::{application::CliViewContext, view::View};
use workyard_core::OperationOutcome;

pub struct RecoverView {
	pub output: CliViewContext,
	pub started: bool,
}

impl View<RecoveryOutcome, RecoveryOutcome, Progress> for RecoverView {
	type Completion = ();
	type Error = io::Error;

	async fn confirm(
		&mut self,
		request: trine::invocation::Confirmation,
		cancellation: trine::Cancellation,
	) -> io::Result<Option<bool>> {
		self.output.confirm(&request, &cancellation)
	}

	async fn progress(&mut self, progress: Progress) -> io::Result<()> {
		let mut terminal = Terminal::for_output(&self.output.stderr);
		if !self.started {
			terminal.heading(&format!("Recover {}", text(&progress.task)));
			self.started = true;
		}
		terminal.progress(&progress);
		self.output.stderr.write_all(terminal.output.as_bytes())
	}

	async fn finish(
		&mut self,
		result: &Result<RecoveryOutcome, RecoveryOutcome>,
	) -> io::Result<()> {
		let outcome = match result {
			Ok(outcome) | Err(outcome) => outcome,
		};
		match outcome {
			RecoveryOutcome::Task(task) => self.finish_outcome(task),
			RecoveryOutcome::Workspace(workspace) => {
				super::workspace::finish(&self.output, workspace)
			}
		}
	}
}

impl RecoverView {
	pub(crate) fn finish_outcome(&mut self, outcome: &RecoverOutcome) -> io::Result<()> {
		let succeeded = outcome.operation.outcome == OperationOutcome::Succeeded;
		let mut terminal = Terminal::for_output(&self.output.stdout);
		if !outcome.pull_failures.is_empty() && outcome.problem.is_none() {
			terminal.completed_with_pull_failures("Recovery", &outcome.task);
		} else if outcome.nothing_to_recover {
			terminal.heading(&format!("Nothing to recover: {}", text(&outcome.task)));
		} else {
			terminal.heading(&format!(
				"Recovery {}: {}",
				outcome.operation.outcome.as_str(),
				text(&outcome.task)
			));
		}

		if !succeeded {
			if outcome.mutation_started == Some(false) {
				terminal.hint_at(MessageScope::Workspace, "No worktrees changed.");
			} else {
				recorded_progress(&mut terminal, outcome);
			}
		}
		terminal.pull_failures(&outcome.pull_failures);
		if let Some(problem) = &outcome.registration_problem {
			terminal.problem(problem);
		}
		if let Some(problem) = &outcome.problem {
			if problem.code != "cancelled" {
				terminal.problem_at(MessageScope::Workspace, problem);
			}
			let needs_inspection = outcome.mutation_started == Some(true)
				|| outcome.condition == Some(crate::contracts::Condition::NeedsAttention)
				|| (outcome.mutation_started.is_none() && !outcome.participants.is_empty());
			if needs_inspection {
				terminal.status_guidance(
					&outcome.task,
					outcome.state_dir.as_ref().map(|path| path.as_path()),
				);
			}
		}
		self.output.stdout.write_all(terminal.output.as_bytes())?;
		Ok(())
	}
}

/// These phases can predate this invocation; never present them as changes it made.
fn recorded_progress(terminal: &mut Terminal, outcome: &RecoverOutcome) {
	if outcome.participants.is_empty() {
		return;
	}
	let participants = outcome
		.participants
		.iter()
		.filter(|participant| participant.phase != "completed")
		.map(|participant| {
			let phase = match participant.phase.as_str() {
				"present" => "present",
				"removed" => "removed",
				"restored" => "restored",
				"restoration_unconfirmed" => "restoration unconfirmed",
				"worktree_created" => "worktree created",
				"pulling" => "pull needs review",
				"not_started" => "creation not started",
				"fetching" => "fetch unconfirmed",
				"creating" => "creation unconfirmed",
				_ => "needs review",
			};
			format!("{} ({phase})", text(&participant.name))
		})
		.collect::<Vec<_>>()
		.join(", ");
	if !participants.is_empty() {
		terminal.hint_at(
			MessageScope::Workspace,
			&format!("Recorded progress: {participants}"),
		);
	}
}

#[cfg(test)]
mod tests;
