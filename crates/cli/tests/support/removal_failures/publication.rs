//! Inject individual publication failures while retaining the real state adapter.
use std::{
	path::{Path, PathBuf},
	sync::atomic::{AtomicBool, Ordering},
};
use workyard_core::*;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Boundary {
	Task,
	FinalEffect,
	SuccessReceipt,
}

pub(super) struct FaultState {
	pub(super) boundary: Boundary,
	pub(super) store: workyard_state::Store,
	pub(super) fail: AtomicBool,
}

impl State for FaultState {
	type Admission = <workyard_state::Store as State>::Admission;
	fn verify(&self) -> Result<(), Problem> {
		self.store.verify()
	}

	fn validate_destination(&self, path: &Path) -> Result<(), Problem> {
		self.store.validate_destination(path)
	}

	fn reserve(&self, path: &Path) -> Result<(), Problem> {
		self.store.reserve(path)
	}

	fn admit(&self, name: &str) -> Result<Self::Admission, Problem> {
		self.store.admit(name)
	}

	fn ensure_new(&self, guard: &Self::Admission, name: &str) -> Result<(), Problem> {
		self.store.ensure_new(guard, name)
	}

	fn record_path(&self, guard: &Self::Admission) -> PathBuf {
		self.store.record_path(guard)
	}

	fn begin(&self, guard: &Self::Admission, task: &Task) -> Result<(), Problem> {
		self.store.begin(guard, task)
	}

	fn save(&self, guard: &Self::Admission, task: &Task) -> Result<(), Problem> {
		let restored = task.removal.as_ref().is_some_and(|removal| {
			removal
				.participants
				.iter()
				.any(|participant| participant.phase == RemovalPhase::Restored)
		});
		if self.boundary == Boundary::Task && restored && self.fail.swap(false, Ordering::SeqCst) {
			return Err(Problem::new(
				ProblemCode::PublicationUncertain,
				"Injected final task publication failure",
			));
		}
		self.store.save(guard, task)
	}

	fn record_effect(
		&self,
		guard: &Self::Admission,
		task: &Task,
		index: usize,
		state: EffectState,
	) -> Result<(), Problem> {
		let phase = task.removal.as_ref().unwrap().participants[index].phase;
		let at_boundary = match self.boundary {
			Boundary::Task => false,
			Boundary::FinalEffect => phase == RemovalPhase::Restored,
			Boundary::SuccessReceipt => phase == RemovalPhase::Recreated,
		};
		if state == EffectState::Settled && at_boundary && self.fail.swap(false, Ordering::SeqCst) {
			return Err(Problem::new(
				ProblemCode::PublicationUncertain,
				"Injected effect publication failure",
			));
		}
		self.store.record_effect(guard, task, index, state)
	}
}

impl RecoveryState for FaultState {
	fn load(&self, guard: &Self::Admission, name: &str) -> Result<RecoveryRecord, Problem> {
		self.store.load(guard, name)
	}
}
