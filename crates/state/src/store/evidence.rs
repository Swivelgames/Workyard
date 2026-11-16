use super::{Store, TaskGuard};
use crate::{TaskRecord, documents::publish, evidence::EffectRecord};
use std::path::PathBuf;
use workyard_core::{Problem, Task};

pub(super) fn effect_path(guard: &TaskGuard) -> PathBuf {
	guard.evidence_path.with_extension("effects")
}

impl Store {
	pub(super) fn publish_effect(
		&self,
		guard: &TaskGuard,
		task: &Task,
		index: usize,
		state: workyard_core::EffectState,
	) -> Result<(), Problem> {
		self.verify()?;
		guard.verify()?;
		let writers = match state {
			workyard_core::EffectState::Pending => crate::revision::WriterUpdate::Pending(index),
			workyard_core::EffectState::Settled => crate::revision::WriterUpdate::Settled,
		};
		if self.revise(guard, task, writers)? {
			return Ok(());
		}
		publish(
			&effect_path(guard),
			&EffectRecord {
				version: 1,
				task: TaskRecord::from(task),
				pending: match state {
					workyard_core::EffectState::Settled => None,
					workyard_core::EffectState::Pending => Some(index),
				},
			},
		)
	}
}
