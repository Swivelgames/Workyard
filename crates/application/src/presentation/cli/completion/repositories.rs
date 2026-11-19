//! Select repository groups before reading their local Git names.
use super::{matching, projects::Lookup};
use std::{
	collections::BTreeSet,
	ffi::{OsStr, OsString},
};
use trine::cli::completion::{Context, OptionValue};
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
	let OptionValue::Present(anchor) = group.field("path") else {
		return None;
	};
	let path = Lookup::new(context).path(anchor, true)?;
	Some(select(&smol::block_on(Catalog::read(&path)).ok()?))
}

fn common(
	context: &Context,
	override_field: &str,
	select: fn(&Catalog) -> BTreeSet<String>,
) -> Option<BTreeSet<String>> {
	let mut intersection: Option<BTreeSet<String>> = None;

	for source in Lookup::new(context).sources()? {
		match source
			.group
			.map_or(OptionValue::Absent, |g| g.field(override_field))
		{
			OptionValue::Present(_) => continue,
			OptionValue::Absent => {}
			OptionValue::Incomplete | OptionValue::Ambiguous => return None,
		}

		let values = select(&smol::block_on(Catalog::read(&source.path)).ok()?);
		match &mut intersection {
			Some(existing) => existing.retain(|value| values.contains(value)),
			None => intersection = Some(values),
		}
	}

	intersection
}

fn branches(context: &Context) -> Option<BTreeSet<String>> {
	let mut values = BTreeSet::new();
	for source in Lookup::new(context).sources()? {
		let remote = match source
			.group
			.map_or(OptionValue::Absent, |g| g.field("remote"))
		{
			OptionValue::Absent => text(context.field("remote"))?,
			value => text(value)?,
		};
		values.extend(
			smol::block_on(Catalog::read(&source.path))
				.ok()?
				.checkout_branches(remote)
				.ok()?,
		);
	}

	Some(values)
}

fn text(value: OptionValue<'_>) -> Option<Option<&str>> {
	match value {
		OptionValue::Absent => Some(None),
		OptionValue::Present(value) => value.to_str().map(Some),
		OptionValue::Incomplete | OptionValue::Ambiguous => None,
	}
}
