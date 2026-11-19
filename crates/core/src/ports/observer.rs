use crate::Problem;
use std::future::Future;

pub trait Observer: Sync {
	fn phase(
		&self,
		task: &str,
		participant: &str,
		phase: &str,
	) -> impl Future<Output = Result<(), Problem>> + Send;
}
