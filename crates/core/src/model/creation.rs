use std::path::PathBuf;

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
