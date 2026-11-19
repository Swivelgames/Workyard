//! Read-only Git-aware discovery. Each registered root reports its own coverage.
use crate::{Runner, output::path_line, process::args};
use std::{
	collections::HashSet,
	ffi::OsString,
	fs, io,
	path::{Path, PathBuf},
};
use workyard_core::{
	Control, Problem, ProblemCode,
	projects::{Catalog, ProjectLocation},
};

pub struct Coverage {
	pub root: PathBuf,
	pub missing: bool,
	pub issues: Vec<Problem>,
}

pub struct Discovery {
	pub catalog: Catalog,
	pub coverage: Vec<Coverage>,
}

impl Discovery {
	pub async fn scan(
		roots: &[PathBuf],
		state: &Path,
		tasks: &Path,
		runner: &Runner,
		control: &Control,
	) -> Result<Self, Problem> {
		let mut locations = Vec::new();
		let mut coverage = Vec::new();
		let scanner = Scanner {
			state,
			tasks,
			runner,
			control,
		};
		for root in roots {
			coverage.push(scanner.root(root, &mut locations).await?);
		}
		Ok(Self {
			catalog: Catalog::new(locations),
			coverage,
		})
	}
}

struct Scanner<'a> {
	state: &'a Path,
	tasks: &'a Path,
	runner: &'a Runner,
	control: &'a Control,
}

impl Scanner<'_> {
	async fn root(
		&self,
		root: &Path,
		locations: &mut Vec<ProjectLocation>,
	) -> Result<Coverage, Problem> {
		let mut report = Coverage {
			root: root.to_owned(),
			missing: false,
			issues: Vec::new(),
		};
		let mut pending = vec![root.to_owned()];
		let mut visited = HashSet::new();
		while let Some(path) = pending.pop() {
			self.control.check()?;
			let directory = match path.canonicalize() {
				Ok(directory) => directory,
				Err(error) if path == root && error.kind() == io::ErrorKind::NotFound => {
					report.missing = true;
					continue;
				}
				Err(error) => {
					report.issues.push(Problem::io(
						format!("Open project directory {path:?}"),
						error,
					));
					continue;
				}
			};
			if directory.starts_with(self.state)
				|| directory.starts_with(self.tasks)
				|| !visited.insert(directory)
			{
				continue;
			}

			match inspect_directory(&path, self.runner, self.control).await {
				Ok(Some(project)) => locations.push(project),
				Ok(None) => collect_children(&path, &mut pending, &mut report.issues),
				Err(problem) if problem.code == ProblemCode::Cancelled => return Err(problem),
				Err(problem) => report.issues.push(problem),
			}
		}
		Ok(report)
	}
}

fn collect_children(directory: &Path, pending: &mut Vec<PathBuf>, issues: &mut Vec<Problem>) {
	let read_error = |error| Problem::io(format!("Read project directory {directory:?}"), error);
	let entries = match fs::read_dir(directory) {
		Ok(entries) => entries,
		Err(error) => {
			issues.push(read_error(error));
			return;
		}
	};
	for entry in entries {
		let child = entry.and_then(|entry| {
			let name = entry.file_name();
			// A directory symlink can leave the registered forest. It can be tracked explicitly.
			if name == ".workyard" || name == ".git" || !entry.file_type()?.is_dir() {
				return Ok(None);
			}
			Ok(Some(entry.path()))
		});
		match child {
			Ok(Some(path)) => pending.push(path),
			Ok(None) => {}
			Err(error) => issues.push(read_error(error)),
		}
	}
}

/// Locate the repository Git would use for an explicitly supplied directory.
pub async fn enclosing_project(
	path: &Path,
	runner: &Runner,
	control: &Control,
) -> Result<Option<ProjectLocation>, Problem> {
	if !path.is_dir() {
		return Ok(None);
	}
	if let Some(project) = inspect_directory(path, runner, control).await? {
		return Ok(Some(project));
	}

	// Let Git find the enclosing worktree through directory symlinks, mount
	// boundaries and its own discovery settings. Its physical root is the source.
	let inside = runner
		.checked(
			path,
			&args(&["rev-parse", "--is-inside-work-tree"]),
			control,
		)
		.await?;
	let root_option = if inside.trim_ascii() == b"true" {
		"--show-toplevel"
	} else {
		"--git-common-dir"
	};
	let output = runner
		.checked(
			path,
			&args(&["rev-parse", "--path-format=absolute", root_option]),
			control,
		)
		.await?;
	let root = path_line(&output)
		.map_err(|_| Problem::new(ProblemCode::InvalidGitOutput, "Invalid Git project root."))?;
	inspect_directory(&root, runner, control).await
}

async fn inspect_directory(
	path: &Path,
	runner: &Runner,
	control: &Control,
) -> Result<Option<ProjectLocation>, Problem> {
	let marker = path.join(".git");
	let git_dir = match fs::symlink_metadata(&marker) {
		Ok(_) => marker,
		Err(error) if error.kind() != io::ErrorKind::NotFound => {
			return Err(Problem::io(format!("Inspect Git marker {marker:?}"), error));
		}
		Err(_) if path.join("HEAD").is_file() && path.join("objects").is_dir() => path.to_owned(),
		Err(_) => return Ok(None),
	};

	// An explicit git-dir keeps Git from discovering an unrelated parent repository.
	let args = [
		OsString::from("--git-dir"),
		git_dir.into_os_string(),
		"rev-parse".into(),
		"--path-format=absolute".into(),
		"--git-common-dir".into(),
	];
	let output = runner.checked(path, &args, control).await?;
	let common = path_line(&output)
		.map_err(|_| Problem::new(ProblemCode::InvalidGitOutput, "Invalid Git project path."))?;
	let identity = workyard_system::directory_identity(&common)?;
	Ok(Some(ProjectLocation {
		identity,
		path: path.to_owned(),
	}))
}
