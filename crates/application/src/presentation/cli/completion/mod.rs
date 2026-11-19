//! Advisory argument values selected from binding-derived context and local metadata.
mod participants;
mod repositories;
pub(crate) mod tasks;

pub(crate) use participants::unsettled_pull;
pub(crate) use repositories::{checkout, common_from, common_remote, repo_from, repo_remote};

use std::{
	collections::BTreeSet,
	ffi::{OsStr, OsString},
};

use trine::cli::completion::{Context, OptionValue};

fn matching(values: Option<BTreeSet<String>>, current: &OsStr) -> Vec<OsString> {
	let Some(prefix) = current.to_str() else {
		return Vec::new();
	};
	values
		.into_iter()
		.flatten()
		.filter(|value| value.starts_with(prefix))
		.map(OsString::from)
		.collect()
}

pub(crate) fn reader(context: &Context) -> Option<workyard_state::Reader> {
	let workspace = match context.option("workspace") {
		OptionValue::Absent => None,
		OptionValue::Present(value) => Some(context.cwd.join(value)),
		OptionValue::Ambiguous | OptionValue::Incomplete => return None,
	};
	crate::workspace::reader(&context.cwd, workspace.as_deref()).ok()?
}
