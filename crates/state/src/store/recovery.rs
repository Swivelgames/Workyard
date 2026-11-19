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
			return Err(
				if !exists(&guard.record_path)?
					&& !exists(&effect_path(guard))?
					&& !exists(&crate::revision::path(&guard.evidence_path))?
				{
					Problem::new(
						workyard_core::ProblemCode::TaskUnknown,
						"Task is unknown in this state directory.",
					)
				} else {
					damaged("No independent admission evidence exists for this task.")
				},
			);
		}
		let intent: TaskRecord =
			crate::documents::read_supported(&guard.evidence_path, &[1, 2, 3, 4])?;
		validate(&intent, name, &self.root, &self.state)?;
		let workspace = self.state.parent().expect("workspace marker");
		let mut relocated_project_roots = intent.relocated_project_roots(workspace)?;

		if let Some(revision) =
			crate::revision::read(&guard.evidence_path, &intent, &self.root, &self.state)?
		{
			let mut needs_publication = true;
			let recorded = if exists(&guard.record_path)? {
				let saved: TaskRecord =
					crate::documents::read_supported(&guard.record_path, &[1, 2, 3, 4])?;
				validate(&saved, name, &self.root, &self.state)?;
				if saved.generation != intent.generation {
					return Err(damaged("Published task belongs to another incarnation."));
				}
				needs_publication = serde_json::to_value(&saved).ok()
					!= serde_json::to_value(&revision.current.task).ok();
				Some(decode(saved, &self.root, &self.state)?)
			} else {
				None
			};
			relocated_project_roots
				.extend(revision.current.task.relocated_project_roots(workspace)?);
			if let Some(request) = revision.request {
				relocated_project_roots.extend(request.relocated_project_roots(workspace)?);
			}
			return Ok(RecoveryRecord {
				relocated_project_roots,
				needs_publication,
				recorded,
				task: decode(revision.current.task, &self.root, &self.state)?,
				pending: revision.current.pending,
			});
		}
		let mut needs_publication = true;
		let recorded = if exists(&guard.record_path)? {
			let (record, version): (TaskRecord, _) =
				crate::documents::read_supported_with_version(&guard.record_path, &[1, 2, 3, 4])?;
			needs_publication = version < 3;
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
			let record = recorded.clone().ok_or_else(|| damaged("Missing task metadata has no independent effect evidence. Restore a verified record; do not recreate worktrees."))?;
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
			relocated_project_roots,
			needs_publication,
			recorded: recorded
				.map(|r| decode(r, &self.root, &self.state))
				.transpose()?,
			task: decode(record, &self.root, &self.state)?,
			pending,
		})
	}
}
