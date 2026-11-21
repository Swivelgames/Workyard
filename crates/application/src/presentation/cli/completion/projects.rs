//! Project references use the same read-only catalog as action preparation.
mod lookup;
use crate::projects::{discover, is_explicit};
pub(super) use lookup::Lookup;
use std::{
	collections::BTreeSet,
	ffi::{OsStr, OsString},
	path::Path,
};
use trine::cli::completion::Context;
use workyard_core::{Control, projects::Catalog};

pub(crate) fn suggest(context: &Context, current: &OsStr) -> Vec<OsString> {
	suggestions(context, current, false)
}

pub(crate) fn repo(context: &Context, current: &OsStr) -> Vec<OsString> {
	suggestions(context, current, true)
}

fn suggestions(context: &Context, current: &OsStr, grouped: bool) -> Vec<OsString> {
	let mut values = BTreeSet::new();
	if grouped || is_explicit(Path::new(current)) {
		values.extend(directories(context, current));
	}
	if is_explicit(Path::new(current)) {
		return values.into_iter().collect();
	}
	let Some(catalog) = inventory(context) else {
		return values.into_iter().collect();
	};
	let selected: BTreeSet<_> = context
		.values("projects")
		.unwrap_or_default()
		.iter()
		.filter_map(|p| catalog.resolve(Path::new(p)).ok().map(|p| &p.path))
		.collect();
	for (reference, project) in catalog.completions(current) {
		if grouped || !selected.contains(&project.path) {
			values.insert(reference.into_os_string());
		}
	}
	values.into_iter().collect()
}

pub(super) fn inventory(context: &Context) -> Option<Catalog> {
	let reader = super::reader(context)?;
	Some(
		smol::block_on(discover(&reader, &Control::default()))
			.ok()?
			.catalog,
	)
}

pub(crate) fn roots(context: &Context, current: &OsStr) -> Vec<OsString> {
	let Some(roots) = super::reader(context).and_then(|r| r.project_roots().ok()) else {
		return Vec::new();
	};
	let (parent, prefix) = directory_prefix(current);
	let typed_parent = context.cwd.join(parent);
	let resolved_parent = workyard_state::canonical_future(&typed_parent).ok();
	let mut suggestions = BTreeSet::new();

	for root in roots {
		let relative = root
			.strip_prefix(&typed_parent)
			.ok()
			.or_else(|| root.strip_prefix(resolved_parent.as_ref()?).ok());
		let candidate = match relative {
			Some(rest) if starts_with(rest.as_os_str(), prefix) => parent.join(rest),
			None if current.is_empty() => root,
			_ => continue,
		};

		let value = if candidate.as_os_str().is_empty() {
			".".into()
		} else {
			candidate.into_os_string()
		};
		suggestions.insert(context.filesystem_candidate(value));
	}
	suggestions.into_iter().collect()
}

fn starts_with(candidate: &OsStr, prefix: &OsStr) -> bool {
	candidate
		.as_encoded_bytes()
		.starts_with(prefix.as_encoded_bytes())
}

/// Match the unfinished name separately from the user's spelling of its parent.
fn directory_prefix(current: &OsStr) -> (&Path, &OsStr) {
	let path = Path::new(current);
	let last_name = current
		.as_encoded_bytes()
		.rsplit(|byte| std::path::is_separator(char::from(*byte)))
		.next()
		.unwrap_or_default();
	if last_name.is_empty() || current == "." || current == ".." {
		return (path, OsStr::new(""));
	}

	let parent = path.parent().unwrap_or(Path::new(""));
	// file_name folds a final "/." and omits a final "/..". Both are typed prefixes.
	match last_name {
		b"." => (path.components().as_path(), OsStr::new(".")),
		b".." => (parent, OsStr::new("..")),
		_ => (parent, path.file_name().unwrap_or_default()),
	}
}

fn directories(context: &Context, current: &OsStr) -> Vec<OsString> {
	let (parent, prefix) = directory_prefix(current);
	let Ok(entries) = std::fs::read_dir(context.cwd.join(parent)) else {
		return Vec::new();
	};
	entries
		.flatten()
		.filter_map(|entry| {
			let name = entry.file_name();
			if !starts_with(&name, prefix) || !entry.path().is_dir() {
				return None;
			}
			let mut candidate = parent.join(name).into_os_string();
			candidate.push(std::path::MAIN_SEPARATOR_STR);
			Some(context.filesystem_candidate(candidate))
		})
		.collect()
}
