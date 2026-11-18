use super::super::interface::Outcome;
use crate::{actions::relocate::RelocateView, contracts::Progress};
use trine::{application::CliViewContext, view::View};

pub(in super::super) struct DispatchView {
	task: super::RecoverView,
	workspace: RelocateView,
}
impl DispatchView {
	pub fn new(output: CliViewContext) -> Self {
		Self {
			task: super::RecoverView {
				output: output.clone(),
				started: false,
			},
			workspace: RelocateView { output },
		}
	}
}
impl View<Outcome, Outcome, Progress> for DispatchView {
	type Completion = ();
	type Error = std::io::Error;
	async fn confirm(
		&mut self,
		request: trine::invocation::Confirmation,
		cancellation: trine::Cancellation,
	) -> std::io::Result<Option<bool>> {
		self.task.confirm(request, cancellation).await
	}
	async fn select(
		&mut self,
		request: trine::invocation::Selection,
		cancellation: trine::Cancellation,
	) -> std::io::Result<Option<usize>> {
		self.workspace.select(request, cancellation).await
	}

	async fn progress(&mut self, progress: Progress) -> std::io::Result<()> {
		if progress.task.is_empty() {
			self.workspace.progress(progress).await
		} else {
			self.task.progress(progress).await
		}
	}
	async fn finish(&mut self, result: &Result<Outcome, Outcome>) -> std::io::Result<()> {
		match result {
			Ok(Outcome::Task(outcome)) | Err(Outcome::Task(outcome)) => {
				self.task.finish_outcome(outcome)
			}
			Ok(Outcome::Workspace(outcome)) | Err(Outcome::Workspace(outcome)) => {
				self.workspace.finish_outcome(outcome)
			}
		}
	}
}
