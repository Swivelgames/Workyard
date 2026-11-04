use serde::Serialize;
use workyard_core::Problem;

#[derive(Serialize)]
pub struct Progress {
	pub task: String,
	pub participant: String,
	pub phase: String,
}

#[derive(Serialize)]
pub struct ProblemOutcome {
	pub code: String,
	pub message: String,
	pub remedy: String,
}

impl From<Problem> for ProblemOutcome {
	fn from(problem: Problem) -> Self {
		Self {
			code: problem.code.as_str().into(),
			message: problem.message,
			remedy: problem.remedy,
		}
	}
}
