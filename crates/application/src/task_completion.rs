//! Advisory recorded-task names; no action execution or participant Git inspection.
use std::{
	collections::BTreeSet,
	ffi::{OsStr, OsString},
	path::PathBuf,
};
use trine::cli::completion::{Context, OptionValue};
use workyard_core::TaskLifecycle;

pub(crate) fn suggest(context: &Context, current: &OsStr) -> Vec<OsString> {
	let Some(prefix) = current.to_str() else {
		return Vec::new();
	};
	let Some(root) = placement(context, "tasks-root") else {
		return Vec::new();
	};
	let Some(explicit) = placement(context, "state-dir") else {
		return Vec::new();
	};
	let start = root.as_deref().unwrap_or(&context.cwd);
	let Ok(Some(state)) = workyard_state::find_state(start, explicit.as_deref()) else {
		return Vec::new();
	};
	let Ok(reader) = workyard_state::Reader::open(&state, root.as_deref()) else {
		return Vec::new();
	};
	let Ok(tasks) = reader.tasks(None) else {
		return Vec::new();
	};

	tasks
		.into_iter()
		.filter_map(|snapshot| {
			let known = snapshot.task.as_ref().or(snapshot.evidence.as_ref());
			if known.is_some_and(|task| task.lifecycle == TaskLifecycle::Retired) {
				return None;
			}
			snapshot
				.name
				.filter(|name| name.starts_with(prefix))
				.map(OsString::from)
		})
		.collect::<BTreeSet<_>>()
		.into_iter()
		.collect()
}

fn placement(context: &Context, option: &str) -> Option<Option<PathBuf>> {
	match context.option(option) {
		OptionValue::Absent => Some(None),
		OptionValue::Present(value) => Some(Some(context.cwd.join(value))),
		OptionValue::Ambiguous => None,
	}
}
