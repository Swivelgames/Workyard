use crate::Problem;
use std::future::Future;

pub trait Observer: Sync {
	fn choose_recovery(
		&self,
		_choice: &crate::RecoveryChoice,
	) -> impl Future<Output = Result<bool, Problem>> + Send {
		async {
			Err(Problem::new(
				crate::ProblemCode::DecisionRequired,
				"Choose the files to recover.",
			))
		}
	}
	fn phase(
		&self,
		task: &str,
		participant: &str,
		phase: &str,
	) -> impl Future<Output = Result<(), Problem>> + Send;
}
