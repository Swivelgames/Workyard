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
	let Some(reader) = reader(context) else {
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

pub(crate) fn reader(context: &Context) -> Option<workyard_state::Reader> {
	let root = placement(context, "tasks-root")?;
	let explicit = placement(context, "state-dir")?;
	let start = root.as_deref().unwrap_or(&context.cwd);
	let state = workyard_state::find_state(start, explicit.as_deref()).ok()??;
	workyard_state::Reader::open(&state, root.as_deref()).ok()
}

fn placement(context: &Context, option: &str) -> Option<Option<PathBuf>> {
	match context.option(option) {
		OptionValue::Absent => Some(None),
		OptionValue::Present(value) => Some(Some(context.cwd.join(value))),
		OptionValue::Ambiguous | OptionValue::Incomplete => None,
	}
}
