//! Advisory recorded-task names; no action execution or participant Git inspection.
use std::{
	collections::BTreeSet,
	ffi::{OsStr, OsString},
};
use trine::cli::completion::Context;
use workyard_core::TaskLifecycle;

pub(crate) fn suggest(context: &Context, current: &OsStr) -> Vec<OsString> {
	let Some(prefix) = current.to_str() else {
		return Vec::new();
	};
	let Some(reader) = super::reader(context) else {
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
