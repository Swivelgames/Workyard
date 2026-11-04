use super::Store;
use crate::documents::read_versioned;
use crate::records::task_filename;
use crate::{Lock, TaskRecord, canonical_future, exists};
use std::{fs, path::PathBuf};
use workyard_core::Problem;

pub struct TaskGuard {
	locks: Vec<Lock>,
	pub record_path: PathBuf,
	pub evidence_path: PathBuf,
}

impl TaskGuard {
	pub(super) fn verify(&self) -> Result<(), Problem> {
		for lock in &self.locks {
			lock.verify()?;
		}

		Ok(())
	}
}

impl Store {
	pub fn admit(&self, name: &str) -> Result<TaskGuard, Problem> {
		self.verify()?;
		// ASCII case aliases share prefix locks on every platform. Unicode
		// filesystem equivalence varies by volume; conservatively exclude other
		// mutations for those names until native equivalence is resolved.
		let spelling = self.state.join("locks").join("path-spelling.lock");
		let mut locks = vec![Lock::acquire(&spelling, name.is_ascii())?];
		let parts: Vec<_> = name.split('/').collect();
		for length in 1..=parts.len() {
			let prefix = parts[..length].join("/").to_ascii_lowercase();
			let path = self
				.state
				.join("locks")
				.join(task_filename(&prefix))
				.with_extension("lock");
			locks.push(Lock::acquire(&path, length < parts.len())?);
		}

		let filename = task_filename(name);
		let guard = TaskGuard {
			locks,
			record_path: self.state.join("tasks").join(&filename),
			evidence_path: self.state.join("evidence").join(filename),
		};
		guard.verify()?;

		Ok(guard)
	}

	pub fn ensure_new(&self, guard: &TaskGuard, name: &str) -> Result<(), Problem> {
		for path in [&guard.record_path, &guard.evidence_path] {
			if !exists(path)? {
				continue;
			}

			let record: TaskRecord = read_versioned(path)?;
			if record.task != name {
				return Err(Problem::new(
					workyard_core::ProblemCode::DamagedState,
					format!("Task identity does not match record {path:?}."),
				));
			}
			let message = format!("Task {name:?} already exists (record/evidence {path:?}).");
			return Err(Problem::new(workyard_core::ProblemCode::TaskExists, message)
				.remedy("Inspect the existing record and its Git worktrees; create/checkout cannot resume or replace it."));
		}

		guard.verify()?;
		self.reject_recorded_overlap(name)?;

		Ok(())
	}

	fn reject_recorded_overlap(&self, name: &str) -> Result<(), Problem> {
		let destination = canonical_future(&self.root.join(name))?;
		for directory in ["tasks", "evidence"] {
			let entries = fs::read_dir(self.state.join(directory))
				.map_err(|error| Problem::io("Enumerate task evidence", error))?;
			for entry in entries {
				let path = entry
					.map_err(|error| Problem::io("Read task evidence entry", error))?
					.path();
				if path.extension().is_none_or(|extension| extension != "json") {
					continue;
				}

				let record: TaskRecord = read_versioned(&path)?;
				if path.file_name() != Some(std::ffi::OsStr::new(&task_filename(&record.task))) {
					return Err(Problem::new(
						workyard_core::ProblemCode::DamagedState,
						format!("Task identity does not match record {path:?}."),
					));
				}

				let existing = canonical_future(&self.root.join(&record.task))?;
				if workyard_core::overlaps(&destination, &existing) {
					return Err(Problem::new(
						workyard_core::ProblemCode::PathOverlap,
						format!(
							"Task destination {destination:?} overlaps recorded task {:?} at {existing:?}.",
							record.task
						),
					)
					.remedy(
						"Choose a task path outside existing task workspaces; preserve their records and worktrees.",
					));
				}
			}
		}
		Ok(())
	}
}
