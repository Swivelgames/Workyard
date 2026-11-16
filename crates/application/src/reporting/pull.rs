//! Invocation-only optional update diagnostics, shared across lifecycle views.
use serde::Serialize;

#[derive(Serialize)]
pub struct PullFailure {
	pub participant: String,
	pub problem: crate::common::ProblemOutcome,
}

impl From<workyard_core::PullFailure> for PullFailure {
	fn from(failure: workyard_core::PullFailure) -> Self {
		Self {
			participant: failure.participant,
			problem: failure.problem.into(),
		}
	}
}
