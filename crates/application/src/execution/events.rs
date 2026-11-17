//! Shared execution boundary and progress adapter for lifecycle actions.
use crate::contracts::Progress;

pub(crate) struct Events(
	pub(crate) trine::InvocationContext<Progress>,
	pub(crate) &'static str,
);

impl workyard_core::Observer for Events {
	async fn phase(
		&self,
		task: &str,
		participant: &str,
		phase: &str,
	) -> Result<(), workyard_core::Problem> {
		self.0
			.emit(Progress {
				disclosure: None,
				action: self.1,
				task: task.into(),
				participant: participant.into(),
				phase: phase.into(),
			})
			.await
			.map_err(|_| {
				workyard_core::Problem::new(
					workyard_core::ProblemCode::Cancelled,
					"Progress delivery ended; operation cancelled.",
				)
			})
	}
}
