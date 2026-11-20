//! Task-local names come from recorded membership, including untracked projects.
use super::matching;
use std::ffi::{OsStr, OsString};
use trine::cli::completion::{Context, OptionValue};
use workyard_core::{Task, TaskLifecycle};

pub(crate) fn task_members(context: &Context, current: &OsStr) -> Vec<OsString> {
	let names = active_task(context)
		.filter(|task| {
			task.removal
				.as_ref()
				.is_none_or(|r| r.restoration_complete())
		})
		.map(|task| task.participants.into_iter().map(|p| p.name).collect());
	matching(names, current)
}

fn active_task(context: &Context) -> Option<Task> {
	let OptionValue::Present(name) = context.field("task") else {
		return None;
	};
	let reader = super::reader(context)?;
	let snapshot = reader.tasks(Some(name.to_str()?)).ok()?.pop()?;
	snapshot
		.evidence
		.or(snapshot.task)
		.filter(|task| task.lifecycle == TaskLifecycle::Active)
}
