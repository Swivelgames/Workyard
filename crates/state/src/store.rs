mod admission;
mod workspace;

pub use admission::TaskGuard;

use crate::documents::publish;
use crate::{Lock, TaskRecord};
use crate::{canonical_future, exists};
use std::{
	fs,
	path::{Path, PathBuf},
};
use workyard_core::{Problem, Task};

pub struct Store {
	root: PathBuf,
	state: PathBuf,
	workspace: Lock,
}

impl Store {
	pub fn begin(&self, guard: &TaskGuard, task: &Task) -> Result<(), Problem> {
		self.verify()?;
		guard.verify()?;
		let record = TaskRecord::from(task);
		// Independent immutable admission evidence survives a missing task document.
		publish(&guard.evidence_path, &record)?;
		publish(&guard.record_path, &record)
	}

	pub fn save(&self, guard: &TaskGuard, task: &Task) -> Result<(), Problem> {
		self.verify()?;
		guard.verify()?;
		publish(&guard.record_path, &TaskRecord::from(task))
	}
}

impl workyard_core::State for Store {
	type Admission = TaskGuard;
	fn verify(&self) -> Result<(), Problem> {
		Store::verify(self)
	}

	fn validate_destination(&self, path: &Path) -> Result<(), Problem> {
		self.verify()?;
		if canonical_future(path)? != path {
			return Err(Problem::new(
				workyard_core::ProblemCode::PathOverlap,
				format!("Destination crosses a changed or aliased ancestor: {path:?}."),
			));
		}

		if exists(path)? {
			return Err(Problem::new(
				workyard_core::ProblemCode::TaskExists,
				format!("Task destination already exists: {path:?}."),
			)
			.remedy(
				"Choose another task name; create/checkout never adopts an existing destination.",
			));
		}

		Ok(())
	}

	fn reserve(&self, path: &Path) -> Result<(), Problem> {
		workyard_core::State::validate_destination(self, path)?;
		let parent = path.parent().expect("validated task destination");
		fs::create_dir_all(parent).map_err(|e| Problem::io("Create task prefix directories", e))?;
		workyard_core::State::validate_destination(self, path)?;
		fs::create_dir(path)
			.map_err(|e| Problem::io(format!("Reserve task destination {path:?}"), e))
	}

	fn admit(&self, name: &str) -> Result<TaskGuard, Problem> {
		Store::admit(self, name)
	}

	fn ensure_new(&self, admission: &TaskGuard, name: &str) -> Result<(), Problem> {
		Store::ensure_new(self, admission, name)
	}

	fn record_path(&self, admission: &TaskGuard) -> PathBuf {
		admission.record_path.clone()
	}

	fn begin(&self, admission: &TaskGuard, task: &Task) -> Result<(), Problem> {
		Store::begin(self, admission, task)
	}

	fn save(&self, admission: &TaskGuard, task: &Task) -> Result<(), Problem> {
		Store::save(self, admission, task)
	}
}
