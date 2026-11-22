use crate::Problem;
use std::future::Future;

/// Receives progress when a task operation reaches a participant step.
pub trait Observer: Sync {
	/// Deliver the named progress step for one task participant.
	fn phase(
		&self,
		task: &str,
		participant: &str,
		phase: &str,
	) -> impl Future<Output = Result<(), Problem>> + Send;
}
