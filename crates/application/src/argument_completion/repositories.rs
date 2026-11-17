//! Select repository groups before reading their local Git names.
use super::matching;
use std::{
	collections::BTreeSet,
	ffi::{OsStr, OsString},
};
use trine::cli::completion::{Context, Group, OptionValue};
use workyard_git::completion::Catalog;

pub(crate) fn repo_remote(context: &Context, current: &OsStr) -> Vec<OsString> {
	matching(per_repo(context, Catalog::remotes), current)
}

pub(crate) fn repo_from(context: &Context, current: &OsStr) -> Vec<OsString> {
	matching(per_repo(context, Catalog::sources), current)
}

pub(crate) fn common_remote(context: &Context, current: &OsStr) -> Vec<OsString> {
	matching(common(context, "remote", Catalog::remotes), current)
}

pub(crate) fn common_from(context: &Context, current: &OsStr) -> Vec<OsString> {
	matching(common(context, "from", Catalog::sources), current)
}

pub(crate) fn checkout(context: &Context, current: &OsStr) -> Vec<OsString> {
	if !matches!(context.field("checkout"), OptionValue::Present(value) if value == "true") {
		return Vec::new();
	}

	matching(branches(context), current)
}

fn per_repo(
	context: &Context,
	select: fn(&Catalog) -> BTreeSet<String>,
) -> Option<BTreeSet<String>> {
	let group = context.current_group("repositories")?;
	Some(select(&catalog(context, group)?))
}

fn common(
	context: &Context,
	override_field: &str,
	select: fn(&Catalog) -> BTreeSet<String>,
) -> Option<BTreeSet<String>> {
	let mut intersection: Option<BTreeSet<String>> = None;

	for group in context.groups("repositories")? {
		match group.field(override_field) {
			OptionValue::Present(_) => continue,
			OptionValue::Absent => {}
			OptionValue::Incomplete | OptionValue::Ambiguous => return None,
		}

		let values = select(&catalog(context, group)?);
		match &mut intersection {
			Some(existing) => existing.retain(|value| values.contains(value)),
			None => intersection = Some(values),
		}
	}

	intersection
}

fn branches(context: &Context) -> Option<BTreeSet<String>> {
	let mut values = BTreeSet::new();
	for group in context.groups("repositories")? {
		let remote = match group.field("remote") {
			OptionValue::Absent => text(context.field("remote"))?,
			value => text(value)?,
		};
		values.extend(catalog(context, group)?.checkout_branches(remote).ok()?);
	}

	Some(values)
}

fn catalog(context: &Context, group: &Group) -> Option<Catalog> {
	let OptionValue::Present(path) = group.field("path") else {
		return None;
	};

	smol::block_on(Catalog::read(&context.cwd.join(path))).ok()
}

fn text(value: OptionValue<'_>) -> Option<Option<&str>> {
	match value {
		OptionValue::Absent => Some(None),
		OptionValue::Present(value) => value.to_str().map(Some),
		OptionValue::Incomplete | OptionValue::Ambiguous => None,
	}
}
