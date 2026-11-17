use super::RemoveOutcome;
use crate::{
	common::Progress,
	terminal::{MessageScope, Terminal, text},
};
use std::io;
use trine::{application::CliViewContext, view::View};
use workyard_core::OperationOutcome;

pub struct RemoveView {
	pub output: CliViewContext,
	pub started: bool,
}

impl View<RemoveOutcome, RemoveOutcome, Progress> for RemoveView {
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
			terminal.heading(&format!("Remove {}", text(&progress.task)));
			self.started = true;
		}
		terminal.progress(&progress);
		self.output.stderr.write_all(terminal.output.as_bytes())
	}

	async fn finish(&mut self, result: &Result<RemoveOutcome, RemoveOutcome>) -> io::Result<()> {
		let outcome = match result {
			Ok(outcome) | Err(outcome) => outcome,
		};
		let succeeded = outcome.operation.outcome == OperationOutcome::Succeeded;
		let mut terminal = Terminal::for_output(&self.output.stdout);
		terminal.heading(&format!(
			"Removal {}: {}",
			outcome.operation.outcome.as_str(),
			text(&outcome.task)
		));

		if outcome.mutation_started == Some(true) && !succeeded {
			partial_summary(&mut terminal, outcome);
		}

		if outcome.mutation_started == Some(false) && !succeeded {
			terminal.hint_at(MessageScope::Workspace, "No worktrees changed.");
		}
		if succeeded {
			terminal.hint_at(MessageScope::Workspace, "Branches and commits retained.");
		}
		if let Some(problem) = &outcome.problem {
			// The result already states ordinary cancellation; retain actionable errors.
			if problem.code != "cancelled" {
				terminal.problem_at(MessageScope::Workspace, problem);
			}
			if outcome.mutation_started == Some(true)
				|| outcome.condition == Some(crate::observation::Condition::NeedsAttention)
			{
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
		terminal.hint_at(MessageScope::Workspace, &format!("{label}: {names}"));
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
		};
		let mut outcome = RemoveOutcome::rejected(
			"feat/task".into(),
			workyard_core::Problem::new(workyard_core::ProblemCode::Cancelled, "cancelled"),
		);
		outcome.mutation_started = Some(true);
		outcome.participants = [("api", "removed"), ("web", "present")]
			.into_iter()
			.map(|(name, phase)| crate::reporting::ParticipantOutcome {
				name: name.into(),
				phase: phase.into(),
				path: NativePath::new(std::path::PathBuf::from(name)).unwrap(),
			})
			.collect();

		smol::block_on(view.finish(&Ok(outcome))).unwrap();
		let output = String::from_utf8(capture.0.lock().unwrap().clone()).unwrap();
		assert!(
			output.contains("Removed: api\nRemaining removal: web\n"),
			"{output}"
		);
		assert!(!output.contains("Repository"));
		assert!(!output.contains("Root:"));
		assert!(!output.contains("cancelled: cancelled"));
	}
}
