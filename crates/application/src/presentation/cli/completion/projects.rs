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
		values.extend(directories(&context.cwd, current));
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
	let prefix = Path::new(current);
	let parent = prefix.parent().unwrap_or(Path::new(""));
	let absolute_parent = workyard_state::canonical_future(&context.cwd.join(parent))
		.unwrap_or_else(|_| context.cwd.join(parent));
	let mut suggestions = BTreeSet::new();
	for root in roots {
		let candidate = if prefix.is_absolute() {
			root
		} else if let Ok(rest) = root.strip_prefix(&absolute_parent) {
			parent.join(rest)
		} else {
			root.strip_prefix(&context.cwd).unwrap_or(&root).to_owned()
		};
		let candidate = if candidate.as_os_str().is_empty() {
			".".into()
		} else {
			candidate.into_os_string()
		};
		if starts_with(&candidate, current) {
			suggestions.insert(candidate);
		}
	}
	suggestions.into_iter().collect()
}

fn starts_with(candidate: &OsStr, prefix: &OsStr) -> bool {
	candidate
		.as_encoded_bytes()
		.starts_with(prefix.as_encoded_bytes())
}

fn directories(cwd: &Path, current: &OsStr) -> Vec<OsString> {
	let path = Path::new(current);
	let ends_with_separator = current
		.as_encoded_bytes()
		.last()
		.is_some_and(|b| std::path::is_separator(char::from(*b)));
	let (parent, prefix) = if ends_with_separator || current == "." || current == ".." {
		(path, OsStr::new(""))
	} else {
		(
			path.parent().unwrap_or(Path::new("")),
			path.file_name().unwrap_or_default(),
		)
	};
	let Ok(entries) = std::fs::read_dir(cwd.join(parent)) else {
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
			Some(candidate)
		})
		.collect()
}
