//! Advisory recorded-task names; no action execution or participant Git inspection.
use std::{collections::BTreeMap, ffi::OsStr};
use trine::cli::completion::{Candidate, Context};
use workyard_core::TaskLifecycle;

pub(crate) fn suggest(context: &Context, current: &OsStr) -> Vec<Candidate> {
	let Some(prefix) = current.to_str() else {
		return Vec::new();
	};
	let Some(reader) = super::reader(context) else {
		return Vec::new();
	};
	let Ok(tasks) = reader.tasks(None) else {
		return Vec::new();
	};

	let mut suggestions = BTreeMap::new();
	for snapshot in tasks {
		let Some(name) = snapshot.name.filter(|name| name.starts_with(prefix)) else {
			continue;
		};
		let task = snapshot.task.or(snapshot.evidence);
		if task
			.as_ref()
			.is_some_and(|task| task.lifecycle == TaskLifecycle::Retired)
		{
			continue;
		}

		let candidate = suggestions
			.entry(name.clone())
			.or_insert_with(|| Candidate::new(name));
		if let Some(task) = task.filter(|task| !task.participants.is_empty()) {
			let names: Vec<_> = task
				.participants
				.into_iter()
				.map(|member| member.name)
				.collect();
			candidate.description = Some(names.join(", "));
		}
	}
	suggestions.into_values().collect()
}
