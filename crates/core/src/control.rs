use crate::Problem;
use std::sync::Arc;

/// Cancellation is supplied by the host; the domain does not choose its runtime.
#[derive(Clone)]
pub struct Control(Arc<dyn Fn() -> bool + Send + Sync>);

impl Control {
	/// Adapt a caller-owned cancellation check for use by lifecycle operations.
	pub fn new(cancelled: impl Fn() -> bool + Send + Sync + 'static) -> Self {
		Self(Arc::new(cancelled))
	}

	/// Query the caller's cancellation signal.
	pub fn cancelled(&self) -> bool {
		(self.0)()
	}

	/// Return a cancellation problem if the caller requested a stop.
	pub fn check(&self) -> Result<(), Problem> {
		if self.cancelled() {
			return Err(Problem::new(
				crate::ProblemCode::Cancelled,
				"Operation cancelled.",
			));
		}

		Ok(())
	}
}

impl Default for Control {
	fn default() -> Self {
		Self::new(|| false)
	}
}
