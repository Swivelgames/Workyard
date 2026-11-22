use crate::{Problem, Task};
use std::path::{Path, PathBuf};

/// Whether managed writers for the recorded effect have stopped.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EffectState {
	/// The supervised Git writer may still be running.
	Pending,
	/// The supervised effect has finished.
	Settled,
}

/// Task admission, publication and effect evidence used by lifecycle operations.
pub trait State: Sync {
	/// Guard held while reading or mutating a specific task.
	type Admission: Send + Sync;
	/// Revalidate the workspace binding before relying on its state.
	fn verify(&self) -> Result<(), Problem>;

	/// Check that the proposed workspace path is available.
	fn validate_destination(&self, path: &Path) -> Result<(), Problem>;

	/// Reserve the proposed destination on the filesystem.
	fn reserve(&self, path: &Path) -> Result<(), Problem>;

	/// Acquire operation admission for the named task.
	fn admit(&self, name: &str) -> Result<Self::Admission, Problem>;

	/// Check under admission that the proposed task can be created.
	fn ensure_new(&self, admission: &Self::Admission, name: &str) -> Result<(), Problem>;

	/// Allocate a fresh incarnation for this task name.
	fn next_generation(&self, _admission: &Self::Admission) -> Result<u64, Problem> {
		Err(Problem::new(
			crate::ProblemCode::IncompatibleState,
			"State adapter must allocate a fresh task generation.",
		))
	}

	/// Clean up retained state belonging to the retired incarnation.
	fn cleanup_retired(&self, _admission: &Self::Admission, _task: &Task) -> Result<(), Problem> {
		Ok(())
	}

	/// Return the authoritative document path for the admitted task.
	fn record_path(&self, admission: &Self::Admission) -> PathBuf;

	/// Publish initial task state while admission is held.
	fn begin(&self, admission: &Self::Admission, task: &Task) -> Result<(), Problem>;

	/// Publish updated task state while admission is held.
	fn save(&self, admission: &Self::Admission, task: &Task) -> Result<(), Problem>;

	/// Named entry point; delegates to the legacy hook for existing adapters.
	fn record_effect(
		&self,
		admission: &Self::Admission,
		task: &Task,
		index: usize,
		state: EffectState,
	) -> Result<(), Problem> {
		self.effect(admission, task, index, state == EffectState::Settled)
	}

	/// Compatibility hook for existing state adapters.
	fn effect(
		&self,
		_admission: &Self::Admission,
		_task: &Task,
		_index: usize,
		_settled: bool,
	) -> Result<(), Problem> {
		Ok(())
	}
}

/// Access to task records and effect evidence for recovery.
pub trait RecoveryState: State {
	/// Reconstruct the admitted task and its outstanding work.
	fn load(
		&self,
		admission: &Self::Admission,
		name: &str,
	) -> Result<crate::RecoveryRecord, Problem>;
}
