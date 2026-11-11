use serde::{Serialize, Serializer};
use workyard_core::{Operation, OperationKind, OperationOutcome};

#[derive(Serialize)]
pub struct OperationReport {
	pub kind: Option<&'static str>,
	pub continues: Option<&'static str>,
	#[serde(serialize_with = "serialize_outcome")]
	pub outcome: OperationOutcome,
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
