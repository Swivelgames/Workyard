//! Install the bundled workspace skill without replacing local content.

use atomicwrites::{AtomicFile, DisallowOverwrite};
use std::{
	fs::{self, Metadata},
	io::{self, Write},
	path::{Component, Path},
};
use workyard_core::{Problem, ProblemCode};

const SKILL: &[u8] = include_bytes!("../../../skills/workyard/SKILL.md");
pub(super) const LOCATION: &str = ".agents/skills/workyard/SKILL.md";

pub(super) fn check_tasks(workspace: &Path, tasks: &Path) -> Result<(), Problem> {
	if tasks.has_root() || matches!(tasks.components().next(), Some(Component::Prefix(_))) {
		return Ok(());
	}

	let tasks = workyard_state::canonical_future(&workspace.join(tasks))?;
	let Ok(relative) = tasks.strip_prefix(workspace) else {
		return Ok(());
	};

	let mut components = relative.components();
	let overlaps = Path::new(LOCATION).components().all(|expected| {
		components.next().is_some_and(|actual| {
			actual
				.as_os_str()
				.eq_ignore_ascii_case(expected.as_os_str())
		})
	});
	if overlaps {
		return Err(Problem::new(
			ProblemCode::PathOverlap,
			format!("The tasks directory overlaps the bundled skill file at {LOCATION}."),
		)
		.remedy("Choose a tasks directory outside the skill file path."));
	}

	Ok(())
}

pub(super) fn install(workspace: &Path) -> io::Result<()> {
	let mut directory = workspace.to_path_buf();
	for name in [".agents", "skills", "workyard"] {
		directory.push(name);
		ensure_directory(&directory)?;
	}

	let path = directory.join("SKILL.md");
	if exists_as(&path, Metadata::is_file)? {
		return Ok(());
	}

	let result = AtomicFile::new(&path, DisallowOverwrite).write(|file| file.write_all(SKILL));
	match result.map_err(io::Error::from) {
		// Another initializer may have published the skill since the lookup.
		Err(error)
			if error.kind() == io::ErrorKind::AlreadyExists
				&& exists_as(&path, Metadata::is_file)? =>
		{
			Ok(())
		}
		result => result,
	}
}

fn ensure_directory(path: &Path) -> io::Result<()> {
	if exists_as(path, Metadata::is_dir)? {
		return Ok(());
	}

	match fs::create_dir(path) {
		Err(error)
			if error.kind() == io::ErrorKind::AlreadyExists
				&& exists_as(path, Metadata::is_dir)? =>
		{
			Ok(())
		}
		result => result,
	}
}

fn exists_as(path: &Path, matches: fn(&Metadata) -> bool) -> io::Result<bool> {
	match fs::symlink_metadata(path) {
		Ok(metadata) if matches(&metadata) => Ok(true),
		Ok(_) => Err(io::Error::new(
			io::ErrorKind::AlreadyExists,
			format!("Unexpected entry at {}.", path.display()),
		)),
		Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(false),
		Err(error) => Err(error),
	}
}
