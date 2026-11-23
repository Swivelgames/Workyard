use super::super::RemoveOutcome;
use crate::{
	contracts::Progress,
	presentation::cli::render::{MessageScope, Terminal, separate, text},
};
use std::io;
use trine::{application::CliViewContext, view::View};
use workyard_core::OperationOutcome;

pub struct RemoveView {
	pub output: CliViewContext,
	pub started: bool,
	pub progress_open: bool,
}

impl View<RemoveOutcome, RemoveOutcome, Progress> for RemoveView {
	type Completion = ();
	type Error = io::Error;

	async fn confirm(
		&mut self,
		request: trine::invocation::Confirmation,
		cancellation: trine::Cancellation,
	) -> io::Result<Option<bool>> {
		let answer = self.output.confirm(&request, &cancellation)?;
		if answer.is_some() {
			// Trine has already separated the prompt from subsequent output.
			self.progress_open = false;
		}
		Ok(answer)
	}

	async fn progress(&mut self, progress: Progress) -> io::Result<()> {
		let mut terminal = Terminal::for_output(&self.output.stderr);
		if !self.started {
			terminal.heading(&format!("Remove {}", text(&progress.task)));
			separate(&mut terminal);
			self.started = true;
		}
		terminal.progress(&progress);
		self.output.stderr.write_all(terminal.output.as_bytes())?;
		self.progress_open = true;
		Ok(())
	}

	async fn finish(&mut self, result: &Result<RemoveOutcome, RemoveOutcome>) -> io::Result<()> {
		let outcome = match result {
			Ok(outcome) | Err(outcome) => outcome,
		};

		let succeeded = outcome.operation.outcome == OperationOutcome::Succeeded;
		if self.progress_open {
			self.output.stderr.write_all(b"\n")?;
		}

		let mut terminal = Terminal::for_output(&self.output.stdout);
		let decision_required = outcome.removal.decision.is_some()
			&& outcome
				.problem
				.as_ref()
				.is_some_and(|problem| problem.code == "decision_required");
		if succeeded
			&& outcome.lifecycle == Some("retired")
			&& outcome.mutation_started == Some(false)
		{
			terminal.heading(&format!("Already retired: {}", outcome.task));
			return self.output.stdout.write_all(terminal.output.as_bytes());
		}

		let title = if decision_required {
			"Removal needs confirmation".into()
		} else {
			format!("Removal {}", outcome.operation.outcome.as_str())
		};
		terminal.heading(&format!("{title}: {}", outcome.task));

		if outcome.mutation_started == Some(true) && !succeeded {
			separate(&mut terminal);
			partial_summary(&mut terminal, outcome);
		}

		if outcome.mutation_started == Some(false) && !succeeded {
			separate(&mut terminal);
			terminal.hint_at(MessageScope::Section, "No worktrees changed.");
		}

		if succeeded {
			separate(&mut terminal);
			terminal.hint_at(MessageScope::Section, "Branches and commits retained.");
		}

		if let Some(problem) = &outcome.problem {
			// The result already states ordinary cancellation; retain actionable errors.
			if decision_required {
				separate(&mut terminal);
				terminal.hint_at(MessageScope::Section, &problem.remedy);
			} else if problem.code != "cancelled" {
				separate(&mut terminal);
				terminal.problem_in_workspace(
					MessageScope::Section,
					problem,
					outcome
						.explicit_workspace
						.as_ref()
						.map(|path| path.as_path()),
				);
			}
			if outcome.mutation_started == Some(true)
				|| outcome.condition == Some(crate::contracts::Condition::NeedsAttention)
			{
				terminal.status_guidance(
					&outcome.task,
					outcome
						.explicit_workspace
						.as_ref()
						.map(|path| path.as_path()),
				);
			}
		}

		self.output.stdout.write_all(terminal.output.as_bytes())?;

		Ok(())
	}
}

fn partial_summary(terminal: &mut Terminal, outcome: &RemoveOutcome) {
	let (removed, remaining): (Vec<_>, Vec<_>) = outcome
		.participants
		.iter()
		.partition(|participant| participant.phase == "removed");

	for (label, participants) in [("Removed", removed), ("Remaining removal", remaining)] {
		if participants.is_empty() {
			continue;
		}

		let names = participants
			.iter()
			.map(|participant| text(&participant.name))
			.collect::<Vec<_>>()
			.join(", ");

		terminal.hint_at(MessageScope::Section, &format!("{label}: {names}"));
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use std::{
		io::Write,
		sync::{Arc, Mutex},
	};
	use trine::{NativePath, application::Output};

	#[derive(Clone, Default)]
	struct Capture(Arc<Mutex<Vec<u8>>>);

	impl Write for Capture {
		fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
			self.0.lock().unwrap().extend_from_slice(bytes);
			Ok(bytes.len())
		}

		fn flush(&mut self) -> io::Result<()> {
			Ok(())
		}
	}

	#[test]
	fn captured_partial_result_retains_changed_and_remaining_participants_without_status_dump() {
		let capture = Capture::default();
		let mut view = RemoveView {
			output: CliViewContext {
				stdout: Output::new(capture.clone()),
				stderr: Output::new(io::sink()),
			},
			started: false,
			progress_open: false,
		};
		let mut outcome = RemoveOutcome::rejected(
			"feat/task".into(),
			workyard_core::Problem::new(workyard_core::ProblemCode::Cancelled, "cancelled"),
		);
		outcome.mutation_started = Some(true);
		outcome.participants = [("api", "removed"), ("web", "present")]
			.into_iter()
			.map(|(name, phase)| crate::contracts::ParticipantOutcome {
				name: name.into(),
				phase: phase.into(),
				path: NativePath::new(std::path::PathBuf::from(name)).unwrap(),
			})
			.collect();

		smol::block_on(view.finish(&Ok(outcome))).unwrap();
		let output = String::from_utf8(capture.0.lock().unwrap().clone()).unwrap();
		assert!(
			output.contains("Removed: api\n    Remaining removal: web\n"),
			"{output}"
		);
		assert!(!output.contains("Repository"));
		assert!(!output.contains("Root:"));
		assert!(!output.contains("cancelled: cancelled"));
	}
}
