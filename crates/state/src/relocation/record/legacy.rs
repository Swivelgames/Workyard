//! Version one embedded a task in each row. Only the indexed participant
//! in that copy received the Git paths observed at admission.
use crate::damage::damaged;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::collections::{BTreeMap, btree_map::Entry};
use workyard_core::Problem;

#[derive(Deserialize, Serialize)]
struct EmbeddedTask {
	task: String,
	participants: Vec<Value>,
	#[serde(flatten)]
	header: Map<String, Value>,
}

impl EmbeddedTask {
	fn merge(&mut self, mut copy: Self, index: usize) -> Result<(), Problem> {
		let same_members = self.participants.len() == copy.participants.len()
			&& self
				.participants
				.iter()
				.zip(&copy.participants)
				.all(|(a, b)| {
					a["name"] == b["name"] && a["backing_identity"] == b["backing_identity"]
				});
		if self.header != copy.header || !same_members {
			return Err(damaged(
				"Embedded relocation records disagree about task identity or membership.",
			));
		}
		self.participants[index] = copy.participants[index].take();
		Ok(())
	}
}

pub(super) fn upgrade(journal: &mut Value) -> Result<(), Problem> {
	if journal.get("tasks").is_some() {
		return Err(damaged(
			"Version one relocation unexpectedly contains a task table.",
		));
	}

	let rows = journal["participants"]
		.as_array_mut()
		.ok_or_else(|| damaged("Missing relocation participants."))?;
	let mut tasks = BTreeMap::new();
	for row in rows {
		let embedded = row
			.get_mut("task")
			.ok_or_else(|| damaged("Missing embedded relocation task."))?;
		let task: EmbeddedTask = serde_json::from_value(embedded.take())
			.map_err(|error| damaged(format!("Invalid embedded relocation task: {error}")))?;
		let index = row["index"]
			.as_u64()
			.and_then(|i| usize::try_from(i).ok())
			.filter(|&i| i < task.participants.len())
			.ok_or_else(|| damaged("Invalid legacy relocation participant index."))?;

		row["task"] = Value::String(task.task.clone());
		match tasks.entry(task.task.clone()) {
			Entry::Vacant(slot) => {
				slot.insert(task);
			}
			Entry::Occupied(mut slot) => slot.get_mut().merge(task, index)?,
		}
	}
	journal["tasks"] = serde_json::to_value(tasks).map_err(|error| damaged(error.to_string()))?;
	journal["version"] = Value::from(2);
	Ok(())
}

#[cfg(test)]
mod tests {
	use super::upgrade;
	use serde_json::json;

	#[test]
	fn keeps_each_rows_own_observation_in_the_shared_task() {
		let task = json!({
			"task": "feat/payments",
			"participants": [
				{"name":"api", "backing_identity":"a", "destination":"old-api"},
				{"name":"web", "backing_identity":"w", "destination":"old-web"}
			]
		});
		let mut api = task.clone();
		api["participants"][0]["destination"] = "observed-api".into();
		let mut web = task;
		web["participants"][1]["destination"] = "observed-web".into();
		let mut journal = json!({"version":1,"participants":[
			{"task":web,"index":1,"absent_at_admission":true},
			{"task":api,"index":0}
		]});

		upgrade(&mut journal).unwrap();
		let members = &journal["tasks"]["feat/payments"]["participants"];
		assert_eq!(members[0]["destination"], "observed-api");
		assert_eq!(members[1]["destination"], "observed-web");
		assert_eq!(
			journal["participants"][0],
			json!({"task":"feat/payments","index":1,"absent_at_admission":true})
		);
	}
}
