use super::inventory;
use crate::projects::{is_explicit, resolve_selector};
use std::{
	ffi::OsStr,
	path::{Path, PathBuf},
};
use trine::cli::completion::{Context, Group, OptionValue};
use workyard_core::projects::Catalog;

pub(in super::super) struct Source<'a> {
	pub path: PathBuf,
	pub group: Option<&'a Group>,
}

/// Resolve selected values once per completion query. Explicit paths also work outside a workspace.
pub(in super::super) struct Lookup<'a> {
	context: &'a Context,
	catalog: Option<Catalog>,
}

impl<'a> Lookup<'a> {
	pub fn new(context: &'a Context) -> Self {
		Self {
			context,
			catalog: None,
		}
	}
	pub fn path(&mut self, selector: &OsStr, grouped: bool) -> Option<PathBuf> {
		let path = Path::new(selector);
		if is_explicit(path) {
			return Some(self.context.cwd.join(path));
		}
		let catalog = self
			.catalog
			.get_or_insert_with(|| inventory(self.context).unwrap_or_default());
		resolve_selector(catalog, &self.context.cwd, path, grouped)
			.ok()
			.map(|project| project.path)
	}
	pub fn sources(&mut self) -> Option<Vec<Source<'a>>> {
		let mut paths = Vec::new();
		for group in self.context.groups("repositories")? {
			let OptionValue::Present(anchor) = group.field("path") else {
				return None;
			};
			paths.push(Source {
				path: self.path(anchor, true)?,
				group: Some(group),
			});
		}
		for project in self.context.values("projects")? {
			let path = self.path(project, false)?;
			if !paths.iter().any(|s| s.path == path) {
				paths.push(Source { path, group: None });
			}
		}
		Some(paths)
	}
}
