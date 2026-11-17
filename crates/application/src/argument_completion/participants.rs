//! Recovery suggestions describe recorded uncertainty, never current Git safety.
use super::matching;
use std::{
	collections::BTreeSet,
	ffi::{OsStr, OsString},
};
use trine::cli::completion::{Context, OptionValue};
use workyard_core::{Phase, TaskLifecycle};

pub(crate) fn unsettled_pull(context: &Context, current: &OsStr) -> Vec<OsString> {
	matching(participants(context), current)
}

fn participants(context: &Context) -> Option<BTreeSet<String>> {
	let OptionValue::Present(name) = context.field("task") else {
		return None;
	};
	let reader = crate::task_completion::reader(context)?;
	let mut snapshots = reader.tasks(Some(name.to_str()?)).ok()?;
	let snapshot = snapshots.pop()?;
	let task = snapshot.evidence.as_ref().or(snapshot.task.as_ref())?;
	if task.lifecycle == TaskLifecycle::Retired || task.removal.is_some() {
		return None;
	}

	Some(
		task.participants
			.iter()
			.filter(|participant| participant.phase == Phase::Pulling)
			.map(|participant| participant.name.clone())
			.collect(),
	)
}
