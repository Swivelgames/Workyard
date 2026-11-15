use crate::{Problem, Task};
use std::path::{Path, PathBuf};

/// Whether managed writers for the recorded effect have stopped.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EffectState {
	Pending,
	Settled,
}

pub trait State: Sync {
	type Admission: Send + Sync;
	fn verify(&self) -> Result<(), Problem>;
	fn validate_destination(&self, path: &Path) -> Result<(), Problem>;
	fn reserve(&self, path: &Path) -> Result<(), Problem>;
	fn admit(&self, name: &str) -> Result<Self::Admission, Problem>;
	fn ensure_new(&self, admission: &Self::Admission, name: &str) -> Result<(), Problem>;

	fn next_generation(&self, _admission: &Self::Admission) -> Result<u64, Problem> {
		Err(Problem::new(
			crate::ProblemCode::IncompatibleState,
			"State adapter must allocate a fresh task generation.",
		))
	}

	fn cleanup_retired(&self, _admission: &Self::Admission, _task: &Task) -> Result<(), Problem> {
		Ok(())
	}

	fn record_path(&self, admission: &Self::Admission) -> PathBuf;
	fn begin(&self, admission: &Self::Admission, task: &Task) -> Result<(), Problem>;
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

pub trait RecoveryState: State {
	fn load(
		&self,
		admission: &Self::Admission,
		name: &str,
	) -> Result<crate::RecoveryRecord, Problem>;
}
