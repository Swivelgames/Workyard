use super::{Store, TaskGuard};
use crate::{TaskRecord, documents::publish, exists, records::task_filename};
use workyard_core::{Problem, RecoveryState, TaskLifecycle};

impl Store {
	/// Archive before replacing any current-generation file. An interruption can
	/// leave mismatched generations, which readers reject rather than guessing.
	pub(super) fn archive_previous(&self, guard: &TaskGuard, name: &str) -> Result<(), Problem> {
		if !exists(&guard.evidence_path)? && !guard.record_path.is_file() {
			return Ok(());
		}
		let previous = self.load(guard, name)?;
		if previous.pending.is_some() || previous.task.lifecycle != TaskLifecycle::Retired {
			return Err(crate::damage::damaged(
				"Cannot replace an active task incarnation.",
			));
		}

		let directory = self.state.join("history").join(task_filename(name));
		std::fs::create_dir_all(&directory)
			.map_err(|error| Problem::io("Create task history directory", error))?;
		let path = directory.join(format!("{}.json", previous.task.generation));
		publish(&path, &TaskRecord::from(&previous.task))
	}
}
