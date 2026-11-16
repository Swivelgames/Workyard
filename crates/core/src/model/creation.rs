use std::path::PathBuf;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CreationOptions {
	pub checkout: bool,
	pub no_fetch: bool,
	pub pull: bool,
}

#[derive(Clone, Debug)]
pub struct RepositoryInput {
	pub path: PathBuf,
	pub name: Option<String>,
	pub remote: Option<String>,
	pub from: Option<String>,
}

#[derive(Clone, Debug)]
pub struct CreateRequest {
	pub task: String,
	pub repositories: Vec<RepositoryInput>,
	pub tasks_root: PathBuf,
	pub state_dir: PathBuf,
	pub checkout: bool,
	pub no_fetch: bool,
	pub pull: bool,
	pub remote: Option<String>,
	pub from: Option<String>,
}

/// An optional Git update failed after membership was established.
#[derive(Clone, Debug)]
pub struct PullFailure {
	pub participant: String,
	pub problem: crate::Problem,
}

pub struct CreateResult {
	pub task: crate::Task,
	pub record: PathBuf,
	pub operation: crate::Operation,
	pub problem: Option<crate::Problem>,
	pub pull_failures: Vec<PullFailure>,
}
