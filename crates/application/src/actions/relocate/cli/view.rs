use super::super::RelocateOutcome;
use crate::{
	contracts::Progress,
	presentation::cli::render::{MessageScope, Terminal},
};
use std::io;
use trine::{application::CliViewContext, view::View};

pub(crate) struct RelocateView {
	pub output: CliViewContext,
}
impl View<RelocateOutcome, RelocateOutcome, Progress> for RelocateView {
	type Completion = ();
	type Error = io::Error;
	async fn confirm(
		&mut self,
		request: trine::invocation::Confirmation,
		cancellation: trine::Cancellation,
	) -> io::Result<Option<bool>> {
		self.output.confirm(&request, &cancellation)
	}
	async fn select(
		&mut self,
		request: trine::invocation::Selection,
		cancellation: trine::Cancellation,
	) -> std::io::Result<Option<usize>> {
		self.output.select(request, &cancellation)
	}

	async fn progress(&mut self, progress: Progress) -> io::Result<()> {
		let mut terminal = Terminal::for_output(&self.output.stderr);
		let stage = match progress.phase.as_str() {
			"preparing" => "Checking workspace relocation",
			"copying_workspace" => "Copying workspace contents",
			"restoring_workspace" => "Restoring original workspace",
			"marking_snapshot" => "Marking the retained copy inactive",
			"moving_workspace" => "Moving workspace contents",
			"repairing_git" => "Verifying Git relationships",
			"transferring_state" => "Transferring authoritative state",
			"cleaning_source" => "Cleaning old state",
			_ => return Ok(()),
		};
		terminal.heading(stage);
		self.output.stderr.write_all(terminal.output.as_bytes())
	}
	async fn finish(
		&mut self,
		result: &Result<RelocateOutcome, RelocateOutcome>,
	) -> io::Result<()> {
		let outcome = match result {
			Ok(outcome) | Err(outcome) => outcome,
		};
		self.finish_outcome(outcome)
	}
}

impl RelocateView {
	pub(crate) fn finish_outcome(&mut self, outcome: &RelocateOutcome) -> io::Result<()> {
		let mut terminal = Terminal::for_output(&self.output.stdout);
		terminal.heading(heading(outcome));
		terminal.workspace(
			outcome
				.active_tasks_root
				.as_ref()
				.or(outcome.new_tasks_root.as_ref())
				.map(|path| path.as_path()),
			outcome
				.active_state_dir
				.as_ref()
				.or(outcome.new_state_dir.as_ref())
				.map(|path| path.as_path()),
		);
		if let Some(snapshot) = &outcome.snapshot {
			let retained = terminal.paths.root(snapshot.tasks_root.as_path());
			terminal.hint_at(
				MessageScope::Workspace,
				&format!("{}: {retained}", snapshot.label()),
			);
		}
		for warning in &outcome.warnings {
			terminal.hint_at(
				MessageScope::Workspace,
				&format!(
					"{}/{} stays at external location {}",
					warning.task,
					warning.participant,
					terminal.paths.root(warning.location.as_path())
				),
			);
		}
		if let Some(problem) = &outcome.problem {
			terminal.problem_at(MessageScope::Workspace, problem);
		}
		if outcome.mutation_started && !outcome.cleanup_complete {
			let state = if outcome.placement_complete {
				&outcome.active_state_dir
			} else {
				&outcome.old_state_dir
			};
			if let Some(path) = state
				.as_ref()
				.and_then(|path| crate::guidance::commands::path(path.as_path()))
			{
				terminal.hint_at(
					MessageScope::Workspace,
					&format!("Continue: workyard recover --state-dir={path}"),
				);
				if !outcome.placement_complete {
					terminal.hint_at(
						MessageScope::Workspace,
						&format!(
							"Return to original placement: workyard recover --state-dir={path} --undo"
						),
					);
				}
			}
		}
		self.output.stdout.write_all(terminal.output.as_bytes())
	}
}

fn heading(outcome: &RelocateOutcome) -> &'static str {
	if outcome.nothing_to_recover {
		return match outcome.operation.kind {
			Some("recover") => "Nothing to recover",
			_ => "Workspace placement is unchanged",
		};
	}
	if let Some(snapshot) = &outcome.snapshot {
		return if snapshot.partial {
			"Original workspace restored"
		} else {
			"Workspace relocated"
		};
	}
	if outcome.placement_complete {
		return match (outcome.cleanup_complete, outcome.copy) {
			(true, _) => "Workspace relocation completed",
			(false, true) => "Workspace placement complete. Snapshot finalization needs attention",
			(false, false) => "Workspace placement completed; source cleanup needs attention",
		};
	}
	if outcome.mutation_started {
		"Workspace relocation is incomplete"
	} else {
		"Relocation did not start; workspace is unchanged"
	}
}
