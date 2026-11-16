use super::Reader;
use crate::{TaskRecord, damage::damaged, records::task::validate, task_filename};
use std::path::Path;
use workyard_core::Problem;

impl Reader {
	pub(super) fn read_task(
		&self,
		path: &Path,
		filename: &Path,
		selected: Option<&str>,
	) -> Result<TaskRecord, Problem> {
		let record: TaskRecord = crate::documents::read_supported(path, &[1, 2, 3, 4])?;
		self.validate_record(&record, filename, selected)?;
		Ok(record)
	}

	pub(super) fn validate_record(
		&self,
		record: &TaskRecord,
		filename: &Path,
		selected: Option<&str>,
	) -> Result<(), Problem> {
		if filename != Path::new(&task_filename(&record.task))
			|| selected.is_some_and(|n| n != record.task)
		{
			return Err(damaged(
				"Task record name does not match its deterministic filename.",
			));
		}
		validate(record, &record.task, &self.root, &self.state)?;
		for participant in &record.participants {
			for path in [
				&participant.source,
				&participant.common_dir,
				&participant.destination,
			] {
				if path.decode()?.as_os_str().as_encoded_bytes().contains(&0) {
					return Err(damaged(
						"Task record contains an unrepresentable native path (NUL).",
					));
				}
			}
		}
		Ok(())
	}
}
