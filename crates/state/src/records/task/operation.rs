use crate::damage::damaged;
use serde::{Deserialize, Serialize};
use workyard_core::{Operation, OperationKind, OperationOutcome, Problem};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OperationRecord {
	pub kind: Option<String>,
	pub continues: Option<String>,
	pub outcome: String,
}

impl From<Operation> for OperationRecord {
	fn from(operation: Operation) -> Self {
		Self {
			kind: operation.kind.map(|kind| kind.as_str().into()),
			continues: operation.continues.map(|kind| kind.as_str().into()),
			outcome: operation.outcome.as_str().into(),
		}
	}
}

impl OperationRecord {
	pub fn decode(&self) -> Result<Operation, Problem> {
		let kind = self.kind.as_deref().map(parse_kind).transpose()?;
		let continues = self.continues.as_deref().map(parse_kind).transpose()?;
		if continues.is_some()
			&& (kind != Some(OperationKind::Recover) || continues == Some(OperationKind::Recover))
		{
			return Err(damaged("Invalid recovery operation provenance."));
		}

		let outcome = match self.outcome.as_str() {
			"pending" => OperationOutcome::Pending,
			"succeeded" => OperationOutcome::Succeeded,
			"failed" => OperationOutcome::Failed,
			"cancelled" => OperationOutcome::Cancelled,
			_ => return Err(damaged("Invalid operation outcome.")),
		};
		Ok(Operation {
			kind,
			continues,
			outcome,
		})
	}
}

fn parse_kind(kind: &str) -> Result<OperationKind, Problem> {
	match kind {
		"create" => Ok(OperationKind::Create),
		"recover" => Ok(OperationKind::Recover),
		"remove" => Ok(OperationKind::Remove),
		_ => Err(damaged("Invalid operation kind.")),
	}
}
