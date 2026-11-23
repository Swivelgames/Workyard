use super::super::CreateOutcome;
use crate::{
	contracts::Progress,
	presentation::cli::render::{MessageScope, TaskTag, Terminal, Tone, separate},
};
use std::{fmt::Write, io};
use trine::{application::CliViewContext, view::View};
use workyard_core::OperationOutcome;

pub struct CreateView {
	pub output: CliViewContext,
	pub wrote_progress: bool,
}

impl View<CreateOutcome, CreateOutcome, Progress> for CreateView {
	type Completion = ();
	type Error = io::Error;

	async fn progress(&mut self, progress: Progress) -> io::Result<()> {
		let activity = match progress.phase.as_str() {
			"fetching" => "fetching",
			"creating" => "creating worktree",
			_ => return Ok(()),
		};

		let mut terminal = Terminal::for_output(&self.output.stderr);
		terminal.note_at(
			MessageScope::Section,
			&format!("{}: {activity}", progress.participant),
		);
		self.output.stderr.write_all(terminal.output.as_bytes())?;
		self.wrote_progress = true;
		Ok(())
	}

	async fn finish(&mut self, result: &Result<CreateOutcome, CreateOutcome>) -> io::Result<()> {
		let outcome = match result {
			Ok(outcome) | Err(outcome) => outcome,
		};

		if self.wrote_progress {
			self.output.stderr.write_all(b"\n")?;
		}

		let mut terminal = Terminal::for_output(&self.output.stdout);

		if outcome.lifecycle.is_none()
			&& let Some(problem) = &outcome.problem
		{
			super::rejection::render(&mut terminal, outcome, problem).await;
		} else {
			render_admitted(&mut terminal, outcome);
		}

		self.output.stdout.write_all(terminal.output.as_bytes())
	}
}

fn render_admitted(terminal: &mut Terminal, outcome: &CreateOutcome) {
	let succeeded = outcome.operation.outcome == OperationOutcome::Succeeded;
	let (title, tone) = match outcome.operation.outcome {
		OperationOutcome::Succeeded => ("Task ready", Tone::SuccessTitle),
		OperationOutcome::Cancelled => ("Task setup cancelled", Tone::Neutral),
		_ => ("Task setup needs attention", Tone::Attention),
	};
	let title = terminal.style.paint(title, tone);
	writeln!(terminal.output, "  {title}\n").expect("string output");

	let tag = if outcome
		.participants
		.iter()
		.all(|participant| participant.phase == "completed")
	{
		TaskTag::Active
	} else {
		TaskTag::NeedsAttention
	};
	terminal.task(&outcome.task, tag, "");
	if !outcome.participants.is_empty() {
		separate(terminal);
	}
	terminal.operation_participants(
		&outcome.participants,
		outcome.tasks_root.as_ref().map(|path| path.as_path()),
		&outcome.task,
		if succeeded {
			"created"
		} else {
			"creation recorded"
		},
	);

	if let Some(problem) = &outcome.registration_problem {
		separate(terminal);
		terminal.problem_at(MessageScope::Section, problem);
	}

	if let Some(problem) = &outcome.problem {
		separate(terminal);
		let workspace = outcome
			.explicit_workspace
			.as_ref()
			.map(|path| path.as_path());
		if !terminal.recorded_task_problem(
			MessageScope::Section,
			problem,
			&outcome.task,
			workspace,
			&outcome.operation,
		) {
			terminal.problem_at(MessageScope::Section, problem);
		}
		if problem.code != "cancelled" && !succeeded {
			terminal.status_guidance(&outcome.task, workspace);
		}
	}
}
