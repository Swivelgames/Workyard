use std::path::PathBuf;

/// Policy shared by initial creation and participant additions.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CreationOptions {
	/// Allow reuse of an existing target branch.
	pub checkout: bool,
	/// Use locally available refs without fetching.
	pub no_fetch: bool,
}

/// Repository selection and its per-participant overrides.
#[derive(Clone, Debug)]
pub struct RepositoryInput {
	/// Explicit source spellings to register once the worktree is admitted.
	pub registration_roots: Vec<PathBuf>,
	/// Backing identity observed when resolving a tracked reference.
	pub expected_identity: Option<String>,
	/// Path to the backing repository selected by the caller.
	pub path: PathBuf,
	/// Optional override for the participant’s directory name inside the task.
	pub name: Option<String>,
	/// Remote selected for branch resolution or fetch.
	pub remote: Option<String>,
	/// Starting revision for a newly created branch.
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

/// Resolved workspace placement, participants and branch options for task creation.
#[derive(Clone, Debug)]
pub struct CreateRequest {
	/// Task selected for this operation.
	pub task: String,
	/// Backing repositories selected for membership.
	pub repositories: Vec<RepositoryInput>,
	/// Directory containing the task workspaces.
	pub tasks_root: PathBuf,
	/// Directory holding authoritative Workyard state.
	pub state_dir: PathBuf,
	/// Allow reuse of an existing target branch.
	pub checkout: bool,
	/// Use locally available refs without fetching.
	pub no_fetch: bool,
	/// Remote selected for branch resolution or fetch.
	pub remote: Option<String>,
	/// Starting revision for a newly created branch.
	pub from: Option<String>,
}

/// Task facts and invocation outcome returned by creation.
pub struct CreateResult {
	/// Task selected for this operation.
	pub task: crate::Task,
	/// Location of the task record when available.
	pub record: PathBuf,
	/// Invocation history tracked separately from task lifecycle.
	pub operation: crate::Operation,
	/// Diagnostic describing a failure or an unresolved condition.
	pub problem: Option<crate::Problem>,
}
