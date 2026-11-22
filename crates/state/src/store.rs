mod admission;
mod amendment;
mod evidence;
mod history;
mod placement;
mod recovery;
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

/// Workspace state adapter for admitted task mutations.
pub struct Store {
	root: PathBuf,
	state: PathBuf,
	workspace: Lock,
}

impl Store {
	/// Publish initial task state while admission is held.
	pub fn begin(&self, guard: &TaskGuard, task: &Task) -> Result<(), Problem> {
		self.verify()?;
		guard.verify()?;
		self.archive_previous(guard, &task.name)?;
		let record = TaskRecord::from(task);
		// Independent immutable admission evidence survives a missing task document.
		publish(&guard.evidence_path, &record)?;
		let revision = crate::revision::path(&guard.evidence_path);
		if exists(&revision)? {
			std::fs::remove_file(revision)
				.map_err(|error| Problem::io("Release archived membership revision", error))?;
		}
		self.publish_effect(guard, task, 0, workyard_core::EffectState::Settled)?;
		publish(&guard.record_path, &record)
	}

	/// Publish updated task state while admission is held.
	pub fn save(&self, guard: &TaskGuard, task: &Task) -> Result<(), Problem> {
		self.verify()?;
		guard.verify()?;
		self.revise(guard, task, crate::revision::WriterUpdate::Preserve)?;
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
		// A missing parent can hide an overlong leaf from stat. Ask the actual
		// destination filesystem about the name before reserving the task.
		if let Some(parent) = path.parent()
			&& !parent.is_dir()
			&& let Some(existing) = parent.ancestors().find(|dir| dir.is_dir())
			&& let Some(name) = path.file_name()
		{
			exists(&existing.join(name)).map_err(|problem| {
				problem.remedy("Choose a shorter task or participant name with --repo-name.")
			})?;
		}
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

	fn next_generation(&self, admission: &TaskGuard) -> Result<u64, Problem> {
		if !exists(&admission.record_path)? {
			return Ok(1);
		}
		let previous = TaskRecord::read(&admission.record_path)?;
		previous
			.generation
			.checked_add(1)
			.ok_or_else(|| crate::damage::damaged("Task generation exhausted."))
	}

	fn cleanup_retired(&self, admission: &TaskGuard, task: &Task) -> Result<(), Problem> {
		self.verify()?;
		admission.verify()?;
		match fs::remove_dir(task.root.join(&task.name)) {
			Ok(()) => Ok(()),
			Err(error)
				if matches!(
					error.kind(),
					std::io::ErrorKind::NotFound | std::io::ErrorKind::DirectoryNotEmpty
				) =>
			{
				Ok(())
			}
			Err(error) => Err(Problem::io("Remove empty retired task directory", error)),
		}
	}

	fn record_path(&self, admission: &TaskGuard) -> PathBuf {
		admission.record_path.clone()
	}

	fn begin(&self, admission: &TaskGuard, task: &Task) -> Result<(), Problem> {
		Store::begin(self, admission, task)
	}

	fn effect(
		&self,
		admission: &TaskGuard,
		task: &Task,
		index: usize,
		settled: bool,
	) -> Result<(), Problem> {
		let state = if settled {
			workyard_core::EffectState::Settled
		} else {
			workyard_core::EffectState::Pending
		};
		self.publish_effect(admission, task, index, state)
	}

	fn save(&self, admission: &TaskGuard, task: &Task) -> Result<(), Problem> {
		Store::save(self, admission, task)
	}
}
