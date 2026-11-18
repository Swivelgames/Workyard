//! Validate and assemble one published task snapshot without rewriting it.
use super::Reader;
use crate::{
	TaskRecord,
	damage::damaged,
	documents::read_versioned,
	evidence::EffectRecord,
	exists,
	records::task::{decode, match_intent},
};
use std::path::Path;
use workyard_core::{Problem, ProblemCode, TaskSnapshot};

impl Reader {
	pub(super) fn snapshot(
		&self,
		filename: impl AsRef<Path>,
		selected: Option<&str>,
	) -> Result<TaskSnapshot, Problem> {
		let filename = filename.as_ref();
		let path = self.state.join("tasks").join(filename);
		let intent_path = self.state.join("evidence").join(filename);
		let mut snapshot = TaskSnapshot {
			name: selected.map(str::to_owned),
			record: path.clone(),
			task: None,
			evidence: None,
			pending: None,
			busy: false,
			problems: Vec::new(),
		};

		let saved = self.read_optional(&path, filename, selected, &mut snapshot);
		if saved.is_none() {
			snapshot.problems.push(damaged("Task record is missing or unreadable. Independent evidence is shown separately; inspect state and run workyard recover with the task and selected state directory."));
		}
		let intent = self.read_optional(&intent_path, filename, selected, &mut snapshot);

		if let Some(intent) = &intent {
			match crate::revision::read(&intent_path, intent, &self.root, &self.state) {
				Ok(Some(revision)) => {
					snapshot.name = Some(revision.current.task.task.clone());
					snapshot.pending = revision.current.pending;
					snapshot.evidence =
						decoded(revision.current.task.clone(), &mut snapshot.problems);
					if saved.is_some() {
						snapshot.task = decoded(revision.current.task, &mut snapshot.problems);
					}
					self.inspect_task_gate(&mut snapshot);
					self.verify()?;
					return Ok(snapshot);
				}
				Ok(None) => {}
				Err(problem) => snapshot.problems.push(problem),
			}
		}
		if let (Some(intent), Some(saved)) = (&intent, &saved)
			&& let Err(problem) = match_intent(intent, saved)
		{
			snapshot.problems.push(problem);
		}

		self.select_evidence(
			&intent_path,
			filename,
			selected,
			intent,
			saved.as_ref(),
			&mut snapshot,
		);
		if let Some(saved) = saved {
			snapshot.task = decoded(saved, &mut snapshot.problems);
		}

		self.inspect_task_gate(&mut snapshot);
		self.verify()?;

		Ok(snapshot)
	}

	fn select_evidence(
		&self,
		intent_path: &Path,
		filename: &Path,
		selected: Option<&str>,
		intent: Option<TaskRecord>,
		saved: Option<&TaskRecord>,
		snapshot: &mut TaskSnapshot,
	) {
		let effects_path = intent_path.with_extension("effects");
		match self.read_effects(&effects_path, filename, selected, intent.as_ref(), saved) {
			Ok(Some(effect)) => {
				if saved.is_some_and(|record| effect_contradicts_success(record, &effect.task)) {
					snapshot.problems.push(damaged("Successful task record conflicts with unfinished or failed effect evidence. Inspect the participant state before running workyard recover."));
				}
				snapshot.name.get_or_insert(effect.task.task.clone());
				snapshot.pending = effect.pending;
				snapshot.evidence = decoded(effect.task, &mut snapshot.problems);
			}
			Ok(None) => {
				if let Some(intent) = intent {
					snapshot.evidence = decoded(intent, &mut snapshot.problems);
				}
			}
			Err(problem) => {
				snapshot.problems.push(problem);
				if let Some(intent) = intent {
					snapshot.evidence = decoded(intent, &mut snapshot.problems);
				}
			}
		}
	}

	fn read_optional(
		&self,
		path: &Path,
		filename: &Path,
		selected: Option<&str>,
		snapshot: &mut TaskSnapshot,
	) -> Option<TaskRecord> {
		let record = match exists(path) {
			Ok(false) => return None,
			Ok(true) => self.read_task(path, filename, selected),
			Err(problem) => Err(problem),
		};
		match record {
			Ok(record) => {
				snapshot.name.get_or_insert(record.task.clone());
				Some(record)
			}
			Err(problem) => {
				snapshot.problems.push(problem);
				None
			}
		}
	}

	fn read_effects(
		&self,
		path: &Path,
		filename: &Path,
		selected: Option<&str>,
		intent: Option<&TaskRecord>,
		saved: Option<&TaskRecord>,
	) -> Result<Option<EffectRecord>, Problem> {
		if !exists(path)? {
			return Ok(None);
		}
		let effect: EffectRecord = read_versioned(path)?;
		self.validate_record(&effect.task, filename, selected)?;
		if effect
			.pending
			.is_some_and(|index| index >= effect.task.participants.len())
		{
			return Err(damaged("Invalid pending participant in effect evidence."));
		}
		for record in [intent, saved].into_iter().flatten() {
			match_intent(record, &effect.task)?;
		}
		Ok(Some(effect))
	}
}

/// Observations retain independently valid facts when a diagnostic is unknown.
/// Recovery uses strict decoding and continues to reject that damaged diagnostic.
fn decoded(mut record: TaskRecord, problems: &mut Vec<Problem>) -> Option<workyard_core::Task> {
	if let Some(problem) = &record.problem
		&& problem.code.parse::<ProblemCode>().is_err()
	{
		problems.push(damaged(format!(
			"Unknown stored problem code {:?}; task facts remain available.",
			problem.code
		)));
		record.problem = None;
	}
	match decode(record) {
		Ok(task) => Some(task),
		Err(problem) => {
			problems.push(problem);
			None
		}
	}
}

/// The last normal effect snapshot can still say pending: success is published
/// afterward. Only concrete unfinished phases contradict that success.
fn effect_contradicts_success(saved: &TaskRecord, effect: &TaskRecord) -> bool {
	if saved.lifecycle == "retired" && effect.lifecycle == "retired" {
		return false;
	}
	if saved.operation.outcome != "succeeded" {
		return false;
	}

	effect
		.participants
		.iter()
		.any(|participant| participant.phase != "completed")
}
