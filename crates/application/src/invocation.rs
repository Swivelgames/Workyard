//! Shared execution boundary and progress adapter for lifecycle actions.
use crate::common::Progress;

pub(crate) struct Events(pub(crate) trine::InvocationContext<Progress>);

impl workyard_core::Observer for Events {
	async fn phase(
		&self,
		task: &str,
		participant: &str,
		phase: &str,
	) -> Result<(), workyard_core::Problem> {
		self.0
			.emit(Progress {
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

pub(crate) async fn blocking_action<F>(operation: F) -> F::Output
where
	F: std::future::Future + Send + 'static,
	F::Output: Send + 'static,
{
	smol::unblock(move || smol::block_on(operation)).await
}
