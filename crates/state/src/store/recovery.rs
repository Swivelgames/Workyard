//! Recover from a membership revision when present, or from the original admission and effect evidence.
use super::evidence::effect_path;
use super::{Store, TaskGuard};
use crate::{
	TaskRecord,
	damage::damaged,
	documents::read_versioned,
	evidence::EffectRecord,
	exists,
	records::task::{decode, match_intent, validate},
	revision::{self, Revision},
};
use std::{collections::BTreeMap, path::PathBuf};
use workyard_core::{Problem, ProblemCode, RecoveryRecord, RecoveryState};

impl RecoveryState for Store {
	fn load(&self, guard: &TaskGuard, name: &str) -> Result<RecoveryRecord, Problem> {
		self.verify()?;
		guard.verify()?;

		let intent = self.read_admission(guard, name)?;
		let workspace = self.state.parent().expect("workspace marker");
		let roots = intent.relocated_project_roots(workspace)?;
		match revision::read(&guard.evidence_path, &intent, &self.root, &self.state)? {
			Some(revision) => self.load_revised_membership(guard, &intent, revision, roots),
			None => self.load_original_membership(guard, &intent, roots),
		}
	}
}

impl Store {
	fn read_admission(&self, guard: &TaskGuard, name: &str) -> Result<TaskRecord, Problem> {
		if exists(&guard.evidence_path)? {
			let intent = TaskRecord::read(&guard.evidence_path)?;
			validate(&intent, name, &self.root, &self.state)?;
			return Ok(intent);
		}

		if exists(&guard.record_path)?
			|| exists(&effect_path(guard))?
			|| exists(&revision::path(&guard.evidence_path))?
		{
			return Err(damaged(
				"No independent admission evidence exists for this task.",
			));
		}
		Err(Problem::new(
			ProblemCode::TaskUnknown,
			"Task is unknown in this state directory.",
		))
	}

	fn load_revised_membership(
		&self,
		guard: &TaskGuard,
		intent: &TaskRecord,
		revision: Revision,
		mut relocated_project_roots: BTreeMap<PathBuf, PathBuf>,
	) -> Result<RecoveryRecord, Problem> {
		let (recorded, needs_publication) = if exists(&guard.record_path)? {
			let saved = TaskRecord::read(&guard.record_path)?;
			validate(&saved, &intent.task, &self.root, &self.state)?;
			if saved.generation != intent.generation {
				return Err(damaged("Published task belongs to another incarnation."));
			}
			let stale = serde_json::to_value(&saved).ok()
				!= serde_json::to_value(&revision.current.task).ok();
			(Some(decode(saved, &self.root, &self.state)?), stale)
		} else {
			(None, true)
		};

		let workspace = self.state.parent().expect("workspace marker");
		relocated_project_roots.extend(revision.current.task.relocated_project_roots(workspace)?);
		if let Some(request) = revision.request {
			relocated_project_roots.extend(request.relocated_project_roots(workspace)?);
		}

		Ok(RecoveryRecord {
			relocated_project_roots,
			needs_publication,
			recorded,
			task: decode(revision.current.task, &self.root, &self.state)?,
			pending: revision.current.pending,
		})
	}

	fn load_original_membership(
		&self,
		guard: &TaskGuard,
		intent: &TaskRecord,
		relocated_project_roots: BTreeMap<PathBuf, PathBuf>,
	) -> Result<RecoveryRecord, Problem> {
		let (recorded, needs_publication) = if exists(&guard.record_path)? {
			let (record, version) = TaskRecord::read_with_version(&guard.record_path)?;
			validate(&record, &intent.task, &self.root, &self.state)?;
			match_intent(intent, &record)?;
			(Some(record), version < 3)
		} else {
			(None, true)
		};

		let effects = effect_path(guard);
		let (record, pending) = if exists(&effects)? {
			let effect: EffectRecord = read_versioned(&effects)?;
			validate(&effect.task, &intent.task, &self.root, &self.state)?;
			match_intent(intent, &effect.task)?;
			if effect
				.pending
				.is_some_and(|index| index >= effect.task.participants.len())
			{
				return Err(damaged("Invalid pending participant in effect evidence."));
			}
			(effect.task, effect.pending)
		} else {
			let Some(record) = recorded.as_ref() else {
				return Err(damaged(
					"Missing task metadata has no independent effect evidence. Restore a verified record; do not recreate worktrees.",
				));
			};
			if record
				.participants
				.iter()
				.any(|p| matches!(p.phase.as_str(), "creating" | "pulling"))
			{
				return Err(damaged(
					"Legacy incomplete effects have no writer settlement evidence. Inspect Git and restore verified operation evidence.",
				));
			}
			(record.clone(), None)
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
