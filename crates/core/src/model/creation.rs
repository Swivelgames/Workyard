use std::path::PathBuf;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CreationOptions {
	pub checkout: bool,
	pub no_fetch: bool,
}

#[derive(Clone, Debug)]
pub struct RepositoryInput {
	/// Explicit source spellings to register once the worktree is admitted.
	pub registration_roots: Vec<PathBuf>,
	/// Backing identity observed when resolving a tracked reference.
	pub expected_identity: Option<String>,
	pub path: PathBuf,
	pub name: Option<String>,
	pub remote: Option<String>,
	pub from: Option<String>,
}

impl RepositoryInput {
	pub(crate) fn verify_identity(
		&self,
		repository: &crate::Repository,
	) -> Result<(), crate::Problem> {
		if self
			.expected_identity
			.as_ref()
			.is_some_and(|expected| expected != &repository.identity)
		{
			return Err(crate::Problem::new(
				crate::ProblemCode::RepositoryChanged,
				format!(
					"The tracked repository at {:?} changed during selection.",
					self.path
				),
			)
			.remedy("Inspect the project location and retry the command."));
		}
		Ok(())
	}
}

#[derive(Clone, Debug)]
pub struct CreateRequest {
	pub task: String,
	pub repositories: Vec<RepositoryInput>,
	pub tasks_root: PathBuf,
	pub state_dir: PathBuf,
	pub checkout: bool,
	pub no_fetch: bool,
	pub remote: Option<String>,
	pub from: Option<String>,
}

pub struct CreateResult {
	pub task: crate::Task,
	pub record: PathBuf,
	pub operation: crate::Operation,
	pub problem: Option<crate::Problem>,
}
