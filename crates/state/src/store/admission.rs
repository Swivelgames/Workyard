use super::Store;
use crate::records::task_filename;
use crate::{Lock, TaskRecord, canonical_future, exists};
use std::{fs, path::PathBuf};
use workyard_core::Problem;

/// Locks and record paths held for one admitted task.
pub struct TaskGuard {
	locks: Vec<Lock>,
	/// Return the authoritative document path for the admitted task.
	pub record_path: PathBuf,
	/// Return the independent effect evidence path for this admission.
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
	/// Acquire operation admission for the named task.
	pub fn admit(&self, name: &str) -> Result<TaskGuard, Problem> {
		self.verify()?;
		// ASCII case aliases share prefix locks on every platform. Unicode
		// filesystem equivalence varies by volume; conservatively exclude other
		// mutations for those names until native equivalence is resolved.
		let spelling = self.state.join("locks").join("path-spelling.lock");
		let spelling_mode = if name.is_ascii() {
			crate::LockMode::Shared
		} else {
			crate::LockMode::Exclusive
		};
		let mut locks = vec![Lock::acquire_mode(&spelling, spelling_mode)?];
		let parts: Vec<_> = name.split('/').collect();
		for length in 1..=parts.len() {
			let prefix = parts[..length].join("/").to_ascii_lowercase();
			let path = self
				.state
				.join("locks")
				.join(task_filename(&prefix))
				.with_extension("lock");
			let prefix_mode = if length < parts.len() {
				crate::LockMode::Shared
			} else {
				crate::LockMode::Exclusive
			};
			locks.push(Lock::acquire_mode(&path, prefix_mode)?);
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

	/// Check under admission that the proposed task can be created.
	pub fn ensure_new(&self, guard: &TaskGuard, name: &str) -> Result<(), Problem> {
		if exists(&guard.record_path)?
			|| exists(&guard.evidence_path)?
			|| exists(&crate::revision::path(&guard.evidence_path))?
		{
			let previous = workyard_core::RecoveryState::load(self, guard, name)?;
			if previous.pending.is_some()
				|| previous.task.lifecycle != workyard_core::TaskLifecycle::Retired
			{
				return Err(Problem::new(workyard_core::ProblemCode::TaskExists,
					format!("Task {name:?} already exists."))
					.remedy("Inspect the task or recover its interrupted operation; create/checkout cannot replace active membership."));
			}
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

				let record = TaskRecord::read(&path)?;
				if path.file_name() != Some(std::ffi::OsStr::new(&task_filename(&record.task))) {
					return Err(Problem::new(
						workyard_core::ProblemCode::DamagedState,
						format!("Task identity does not match record {path:?}."),
					));
				}

				// A retired current record releases its path, including its immutable
				// admission evidence. A missing or inconsistent current record does not.
				let current_path = self.state.join("tasks").join(task_filename(&record.task));
				if exists(&current_path)? {
					let current = TaskRecord::read(&current_path)?;
					crate::records::task::validate(
						&current,
						&record.task,
						&self.root,
						&self.state,
					)?;
					let intent_path = self
						.state
						.join("evidence")
						.join(task_filename(&record.task));
					let intent = TaskRecord::read(&intent_path)?;
					if let Some(revision) =
						crate::revision::read(&intent_path, &intent, &self.root, &self.state)?
					{
						if revision.current.task.lifecycle == "retired" {
							continue;
						}
					} else {
						crate::records::task::match_intent(&record, &current)?;
					}
					if current.lifecycle == "retired" {
						continue;
					}
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
