use super::Reader;
use crate::{exists, task_filename};
use std::{collections::BTreeSet, path::Path};
use workyard_core::{Problem, ProblemCode, TaskSnapshot};

impl Reader {
	/// Read task snapshots for inspection.
	pub fn tasks(&self, selected: Option<&str>) -> Result<Vec<TaskSnapshot>, Problem> {
		self.verify()?;
		if let Some(name) = selected {
			workyard_core::validate_task(name)?;
			let filename = task_filename(name);
			let record = self.state.join("tasks").join(&filename);
			let intent = self.state.join("evidence").join(&filename);
			let revision = intent.with_extension("revision");
			let effects = intent.with_extension("effects");
			if !exists(&record)? && !exists(&intent)? && !exists(&revision)? && !exists(&effects)? {
				return Err(Problem::new(
					ProblemCode::TaskUnknown,
					format!("Task {name:?} is unknown in this workspace."),
				)
				.remedy("Run workyard list in the same workspace to see known tasks."));
			}
			return Ok(vec![self.snapshot(&filename, Some(name))?]);
		}
		let mut filenames = BTreeSet::new();
		for directory in ["tasks", "evidence"] {
			for entry in std::fs::read_dir(self.state.join(directory))
				.map_err(|e| Problem::io("Enumerate task records", e))?
			{
				let path = entry
					.map_err(|e| Problem::io("Read record entry", e))?
					.path();
				if path.extension().is_some_and(|e| {
					e == "json" || (directory == "evidence" && (e == "effects" || e == "revision"))
				}) {
					filenames.insert(
						path.with_extension("json")
							.file_name()
							.expect("record name")
							.to_owned(),
					);
				}
			}
		}
		let mut tasks = Vec::new();
		for filename in filenames {
			tasks.push(self.snapshot(Path::new(&filename), None)?);
		}
		tasks.sort_by(|a, b| a.name.cmp(&b.name).then(a.record.cmp(&b.record)));
		self.verify()?;
		Ok(tasks)
	}
}
