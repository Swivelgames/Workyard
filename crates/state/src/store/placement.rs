//! Refresh only location fields. Lifecycle and writer evidence may differ by document.
use super::{Store, TaskGuard, evidence::effect_path};
use crate::{
	TaskRecord,
	documents::{publish, read_versioned},
	evidence::EffectRecord,
	exists, revision,
};
use workyard_core::{Participant, Problem};

impl Store {
	pub fn remember_participant(
		&self,
		guard: &TaskGuard,
		participant: &Participant,
	) -> Result<bool, Problem> {
		self.verify()?;
		guard.verify()?;
		let anchor = TaskRecord::read(&guard.evidence_path)?;
		let mut changed = false;
		if let Some(mut revised) =
			revision::read(&guard.evidence_path, &anchor, &self.root, &self.state)?
		{
			if revised
				.current
				.task
				.refresh_location(participant, &self.state)?
			{
				publish(&revision::path(&guard.evidence_path), &revised)?;
				changed = true;
			}
		} else {
			let effects = effect_path(guard);
			if exists(&effects)? {
				let mut record: EffectRecord = read_versioned(&effects)?;
				if record.task.refresh_location(participant, &self.state)? {
					publish(&effects, &record)?;
					changed = true;
				}
			}
		}
		let mut published = TaskRecord::read(&guard.record_path)?;
		if published.refresh_location(participant, &self.state)? {
			publish(&guard.record_path, &published)?;
			changed = true;
		}
		Ok(changed)
	}
}
