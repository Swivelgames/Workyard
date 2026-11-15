//! Which task records each read-only command includes.
use workyard_core::{Problem, TaskSnapshot};
use workyard_state::Reader;

#[derive(Clone, Copy)]
pub(crate) enum Selection {
	Inventory,
	Active,
}

impl Selection {
	pub fn snapshots(
		self,
		reader: &Reader,
		task: Option<&str>,
	) -> Result<Vec<TaskSnapshot>, Problem> {
		let mut snapshots = reader.tasks(task)?;
		if matches!(self, Selection::Inventory) {
			for history in reader.history()? {
				let duplicate = snapshots.iter().any(|current| {
					current
						.task
						.as_ref()
						.zip(history.task.as_ref())
						.is_some_and(|(a, b)| a.name == b.name && a.generation == b.generation)
				});
				if !duplicate {
					snapshots.push(history);
				}
			}
			snapshots.sort_by_key(|snapshot| {
				std::cmp::Reverse(
					snapshot
						.task
						.as_ref()
						.and_then(|task| task.removal.as_ref())
						.and_then(|removal| removal.retired_at),
				)
			});
			let mut retired = 0;
			snapshots.retain(|snapshot| {
				if snapshot
					.task
					.as_ref()
					.is_some_and(|task| task.lifecycle == workyard_core::TaskLifecycle::Retired)
				{
					retired += 1;
					return retired <= 5;
				}
				true
			});
		}
		if matches!(self, Selection::Active) {
			snapshots.retain(|snapshot| {
				!snapshot
					.task
					.as_ref()
					.is_some_and(|task| task.lifecycle == workyard_core::TaskLifecycle::Retired)
					|| !snapshot.problems.is_empty()
					|| snapshot.pending.is_some()
			});
		}

		Ok(snapshots)
	}
}
