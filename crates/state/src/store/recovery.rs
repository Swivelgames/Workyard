use super::evidence::effect_path;
use super::{Store, TaskGuard};
use crate::documents::read_versioned;
use crate::records::task::{decode, match_intent, validate};
use crate::{TaskRecord, exists};
use crate::{damage::damaged, evidence::EffectRecord};
use workyard_core::{Problem, RecoveryRecord, RecoveryState};

impl RecoveryState for Store {
	fn load(&self, guard: &TaskGuard, name: &str) -> Result<RecoveryRecord, Problem> {
		self.verify()?;
		guard.verify()?;
		if !exists(&guard.evidence_path)? {
			return Err(damaged(
				"No independent admission evidence exists for this task.",
			));
		}
		let intent: TaskRecord = crate::documents::read_supported(&guard.evidence_path, &[1, 2])?;
		validate(&intent, name, &self.root, &self.state)?;
		let recorded = if exists(&guard.record_path)? {
			let record: TaskRecord = crate::documents::read_supported(&guard.record_path, &[1, 2])?;
			validate(&record, name, &self.root, &self.state)?;
			match_intent(&intent, &record)?;
			Some(record)
		} else {
			None
		};

		let effects = effect_path(guard);
		let (record, pending) = if exists(&effects)? {
			let effect: EffectRecord = read_versioned(&effects)?;
			validate(&effect.task, name, &self.root, &self.state)?;
			match_intent(&intent, &effect.task)?;
			if effect
				.pending
				.is_some_and(|index| index >= effect.task.participants.len())
			{
				return Err(damaged("Invalid pending participant in effect evidence."));
			}
			(effect.task, effect.pending)
		} else {
			let record = recorded.ok_or_else(|| damaged("Missing task metadata has no independent effect evidence. Restore a verified record; do not recreate worktrees."))?;
			if record
				.participants
				.iter()
				.any(|p| matches!(p.phase.as_str(), "creating" | "pulling"))
			{
				return Err(damaged(
					"Legacy incomplete effects have no writer settlement evidence. Inspect Git and restore verified operation evidence.",
				));
			}
			(record, None)
		};
		Ok(RecoveryRecord {
			task: decode(record)?,
			pending,
		})
	}
}
