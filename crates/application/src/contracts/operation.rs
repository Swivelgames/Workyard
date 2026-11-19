use serde::{Serialize, Serializer};
use workyard_core::{Operation, OperationKind, OperationOutcome, Problem, ProblemCode};

#[derive(Serialize)]
pub struct OperationReport {
	pub kind: Option<&'static str>,
	pub continues: Option<&'static str>,
	#[serde(serialize_with = "serialize_outcome")]
	pub outcome: OperationOutcome,
}

impl OperationReport {
	pub(crate) fn rejected(kind: OperationKind, problem: &Problem) -> Self {
		let outcome = match problem.code {
			ProblemCode::Cancelled => OperationOutcome::Cancelled,
			_ => OperationOutcome::Failed,
		};

		Operation {
			kind: Some(kind),
			continues: None,
			outcome,
		}
		.into()
	}
}

impl From<Operation> for OperationReport {
	fn from(operation: Operation) -> Self {
		Self {
			kind: operation.kind.map(OperationKind::as_str),
			continues: operation.continues.map(OperationKind::as_str),
			outcome: operation.outcome,
		}
	}
}

fn serialize_outcome<S: Serializer>(
	outcome: &OperationOutcome,
	serializer: S,
) -> Result<S::Ok, S::Error> {
	serializer.serialize_str(outcome.as_str())
}
