//! Normalize legacy task documents in memory; readers never migrate files.
use super::{OperationRecord, ParticipantRecord, ProblemRecord, TaskRecord};
use crate::path::StoredPath;
use serde::{Deserialize, Deserializer, de::Error};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Document {
	version: u32,
	#[serde(default)]
	generation: u64,
	#[serde(default)]
	removal: Option<super::RemovalRecord>,
	task: String,
	tasks_root: StoredPath,
	state_dir: StoredPath,
	checkout: bool,
	no_fetch: bool,
	pull: bool,
	#[serde(default)]
	status: Option<String>,
	#[serde(default)]
	lifecycle: Option<String>,
	#[serde(default)]
	operation: Option<OperationRecord>,
	participants: Vec<ParticipantRecord>,
	problem: Option<ProblemRecord>,
}

impl<'de> Deserialize<'de> for TaskRecord {
	fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
		let value = serde_json::Value::deserialize(deserializer)?;
		let version = value.get("version").and_then(serde_json::Value::as_u64);
		let mixed = match version {
			Some(1) => {
				value.get("lifecycle").is_some()
					|| value.get("operation").is_some()
					|| value.get("generation").is_some()
					|| value.get("removal").is_some()
			}
			Some(2) => {
				value.get("status").is_some()
					|| value.get("generation").is_some()
					|| value.get("removal").is_some()
			}
			Some(3 | 4) => {
				value.get("status").is_some()
					|| value.get("generation").is_none()
					|| value.get("removal").is_none()
			}
			_ => true,
		};
		if mixed {
			return Err(D::Error::custom(
				"Unsupported or mixed task document format",
			));
		}
		let document: Document = serde_json::from_value(value).map_err(D::Error::custom)?;
		let (lifecycle, operation) = match document.version {
			1 if document.lifecycle.is_none() && document.operation.is_none() => {
				let status = document
					.status
					.as_deref()
					.ok_or_else(|| D::Error::custom("Missing legacy task status"))?;
				let outcome = match status {
					"completed" => "succeeded",
					"pending" | "failed" | "cancelled" => status,
					_ => return Err(D::Error::custom("Invalid legacy task status")),
				};
				(
					"active".into(),
					OperationRecord {
						kind: None,
						continues: None,
						outcome: outcome.into(),
					},
				)
			}
			2..=4 if document.status.is_none() => {
				let lifecycle = document
					.lifecycle
					.ok_or_else(|| D::Error::custom("Missing task lifecycle"))?;
				let operation = document
					.operation
					.ok_or_else(|| D::Error::custom("Missing task operation"))?;
				(lifecycle, operation)
			}
			_ => {
				return Err(D::Error::custom(
					"Unsupported or mixed task document format",
				));
			}
		};

		Ok(TaskRecord {
			// In-memory normalization also makes subsequent serialization canonical.
			version: document.version.max(3),
			generation: document.generation,
			removal: document.removal,
			task: document.task,
			tasks_root: document.tasks_root,
			state_dir: document.state_dir,
			checkout: document.checkout,
			no_fetch: document.no_fetch,
			pull: document.pull,
			lifecycle,
			operation,
			participants: document.participants,
			problem: document.problem,
		})
	}
}
