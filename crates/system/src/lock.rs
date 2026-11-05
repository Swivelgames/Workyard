use crate::file_identity;
use std::{
	fs::{File, OpenOptions},
	path::{Path, PathBuf},
	time::Duration,
};
use workyard_core::{Control, Problem};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LockMode {
	Shared,
	Exclusive,
}

/// Locks a stable coordination inode, never a replaceable JSON document.
pub struct Lock {
	file: File,
	path: PathBuf,
}

impl Lock {
	/// Compatibility entry point; new callers can make the mode explicit.
	pub fn acquire(path: &Path, shared: bool) -> Result<Self, Problem> {
		let mode = if shared {
			LockMode::Shared
		} else {
			LockMode::Exclusive
		};
		Self::acquire_mode(path, mode)
	}

	pub fn acquire_mode(path: &Path, mode: LockMode) -> Result<Self, Problem> {
		let file = OpenOptions::new()
			.create(true)
			.truncate(false)
			.read(true)
			.write(true)
			.open(path)
			.map_err(|e| Problem::io(format!("Open lock {path:?}"), e))?;
		let attempt = match mode {
			LockMode::Shared => file.try_lock_shared(),
			LockMode::Exclusive => file.try_lock(),
		};
		attempt.map_err(|error| match error {
			std::fs::TryLockError::WouldBlock => Problem::new(
				workyard_core::ProblemCode::ResourceBusy,
				format!("Resource is busy: {path:?}"),
			)
			.remedy(
				"Wait for the active Workyard operation to finish, then retry. Do not delete lockfiles.",
			),
			std::fs::TryLockError::Error(error) => Problem::io(format!("Lock {path:?}"), error),
		})?;
		let lock = Self {
			file,
			path: path.into(),
		};
		lock.verify()?;
		Ok(lock)
	}

	pub async fn wait(path: &Path, control: &Control) -> Result<Self, Problem> {
		loop {
			control.check()?;
			match Self::acquire_mode(path, LockMode::Exclusive) {
				Ok(lock) => return Ok(lock),
				Err(error) if error.code == workyard_core::ProblemCode::ResourceBusy => {}
				Err(error) => return Err(error),
			}
			smol::Timer::after(Duration::from_millis(20)).await;
		}
	}

	pub fn verify(&self) -> Result<(), Problem> {
		let named = File::open(&self.path).map_err(|e| Problem::io("Open named lock", e))?;
		let same = file_identity(&self.file)? == file_identity(&named)?;
		if !same {
			return Err(Problem::new(
				workyard_core::ProblemCode::StaleIdentity,
				format!("Lock was replaced: {:?}", self.path),
			)
			.remedy(
				"Stop conflicting processes and inspect the state directory before retrying.",
			));
		}

		Ok(())
	}
}

impl Drop for Lock {
	fn drop(&mut self) {
		// End the guard's ownership explicitly. On Unix, another thread's
		// fork-before-exec can briefly retain a copy of this file description;
		// closing our descriptor alone would prolong its flock until exec.
		let _ = self.file.unlock();
	}
}

#[cfg(all(test, unix))]
mod tests {
	#[test]
	fn dropping_guard_releases_lock_even_with_a_duplicate_descriptor() {
		let directory = tempfile::tempdir().unwrap();
		let path = directory.path().join("resource.lock");
		let guard = super::Lock::acquire(&path, false).unwrap();
		// A fork-before-exec inherits this same open file description.
		let inherited = guard.file.try_clone().unwrap();
		drop(guard);

		let next = super::Lock::acquire(&path, false).unwrap();
		drop(inherited);
		next.verify().unwrap();
	}
}
