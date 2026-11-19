use serde::Serialize;
use trine::NativePath;

use super::Condition;

#[derive(Clone, Serialize)]
pub struct Disclosure {
	pub participant: String,
	pub dirty_paths: Vec<NativePath>,
	pub history: &'static str,
}

#[derive(Clone, Serialize)]
pub struct DecisionRequirement {
	#[serde(skip_serializing_if = "Option::is_none")]
	pub task: Option<String>,
	pub kind: String,
	#[serde(skip_serializing_if = "Vec::is_empty")]
	pub participants: Vec<String>,
	pub participant: String,
	pub message: String,
}

#[derive(Default, Serialize)]
pub struct RemovalReport {
	#[serde(skip_serializing_if = "Vec::is_empty")]
	pub disclosures: Vec<Disclosure>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub decision: Option<DecisionRequirement>,
}

/// Cancellation is an invocation outcome; task condition comes from known facts.
pub(crate) fn condition(result: &workyard_core::RemovalResult) -> Condition {
	if result.mutation_started {
		return if result.operation.outcome == workyard_core::OperationOutcome::Succeeded {
			Condition::Healthy
		} else {
			Condition::NeedsAttention
		};
	}

	if result.needs_attention {
		return Condition::NeedsAttention;
	}

	if result.inspected && result.record_available {
		return Condition::Healthy;
	}

	Condition::Unknown
}
