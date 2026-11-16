//! Historical summaries are independent immutable incarnations, never admissions.
use super::Reader;
use crate::{
	TaskRecord,
	documents::read_supported,
	exists,
	records::task::{decode, validate},
};
use workyard_core::{Problem, TaskSnapshot};

impl Reader {
	pub fn history(&self) -> Result<Vec<TaskSnapshot>, Problem> {
		self.verify()?;
		let directory = self.state.join("history");
		if !exists(&directory)? {
			return Ok(Vec::new());
		}
		let mut records = Vec::new();
		for entry in
			std::fs::read_dir(&directory).map_err(|e| Problem::io("Read task history", e))?
		{
			let directory = entry
				.map_err(|e| Problem::io("Read task history entry", e))?
				.path();
			if !directory.is_dir() {
				continue;
			}
			for entry in std::fs::read_dir(&directory)
				.map_err(|e| Problem::io("Read task incarnations", e))?
			{
				let path = entry
					.map_err(|e| Problem::io("Read incarnation entry", e))?
					.path();
				if path.extension().is_none_or(|extension| extension != "json") {
					continue;
				}
				let record: TaskRecord = read_supported(&path, &[3, 4])?;
				validate(&record, &record.task, &self.root, &self.state)?;
				if record.lifecycle != "retired"
					|| directory.file_name()
						!= Some(std::ffi::OsStr::new(&crate::task_filename(&record.task)))
					|| path.file_name()
						!= Some(std::ffi::OsStr::new(&format!("{}.json", record.generation)))
				{
					return Err(crate::damage::damaged(
						"Invalid retired incarnation identity.",
					));
				}
				let task = decode(record)?;
				records.push(TaskSnapshot {
					name: Some(task.name.clone()),
					record: path,
					task: Some(task),
					evidence: None,
					pending: None,
					busy: false,
					problems: Vec::new(),
				});
			}
		}
		self.verify()?;
		Ok(records)
	}
}
