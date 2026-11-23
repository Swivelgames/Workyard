use std::path::PathBuf;

/// Structured guidance for an ambiguous or conflicting project selection.
#[derive(Clone, Debug)]
pub enum ProjectRemedy {
	/// Select one of the matching project references.
	Choose(Vec<PathBuf>),
	/// Supply a unique name for the participant.
	Rename {
		/// Backing repository whose participant needs a distinct name.
		source: PathBuf,
		/// Proposed replacement participant name.
		name: String,
	},
}

/// Locations to inspect when Git and recorded placement disagree.
#[derive(Clone, Debug)]
pub struct WorktreeLocations {
	/// Location reported by Git registration.
	pub registered: PathBuf,
	/// Possible current location requiring verification.
	pub candidate: PathBuf,
}

/// Additional facts that a presenter can use without parsing diagnostic prose.
#[derive(Clone, Debug)]
pub enum ProblemDetail {
	/// Captured Git diagnostics, separate from the command invocation.
	GitDiagnostic(String),
	/// User-supplied starting point that a repository could not resolve.
	StartingPoint(String),
	/// A previously admitted worktree could not be located.
	MissingWorktree {
		/// Name by which the task identifies this worktree.
		participant: String,
		/// Worktree location currently expected by Workyard.
		expected: PathBuf,
		/// Whether the expected path is occupied by an unverified entry.
		occupied: bool,
	},
	/// An initialized workspace is already configured to use a different task directory.
	TaskDirectoryConflict {
		/// Workspace containing the configured tasks directory.
		workspace: PathBuf,
		/// Configured tasks directory.
		current: PathBuf,
	},
}

/// Classified failure with diagnostic text and optional recovery guidance.
#[derive(Clone, Debug)]
pub struct Problem {
	/// Paths associated with a worktree relationship problem.
	pub worktree_locations: Option<Box<WorktreeLocations>>,
	/// Stable machine-readable diagnostic classification.
	pub code: crate::ProblemCode,
	/// Human-readable explanation for the caller.
	pub message: String,
	/// Suggested action for resolving the problem.
	pub remedy: String,
	/// Structured project selection or naming guidance.
	pub project_remedy: Option<Box<ProjectRemedy>>,
	/// Repositories affected by this diagnostic, for presentation by native path.
	pub repositories: Vec<PathBuf>,
	/// Additional presentation facts available for this failure.
	pub detail: Option<Box<ProblemDetail>>,
}

impl Problem {
	/// Create a classified diagnostic with the supplied message.
	pub fn new(code: crate::ProblemCode, message: impl Into<String>) -> Self {
		Self {
			code,
			message: message.into(),
			remedy: String::new(),
			project_remedy: None,
			worktree_locations: None,
			repositories: Vec::new(),
			detail: None,
		}
	}

	/// Attach a suggested next step for the caller.
	pub fn remedy(mut self, remedy: impl Into<String>) -> Self {
		self.remedy = remedy.into();
		self
	}

	/// Identify the repository associated with a diagnostic.
	pub fn in_repository(mut self, path: &std::path::Path) -> Self {
		self.repositories.push(path.into());
		self
	}

	/// A later invocation cannot infer process exit from an available Workyard lock.
	pub(crate) fn unconfirmed_git_exit(context: impl std::fmt::Display) -> Self {
		let message = format!("Workyard cannot confirm Git finished {context}.");
		let remedy = "You can inspect files. Leave the worktree in place while checking Git and its hooks. \
			If the original Workyard process has exited, retrying cannot clear this interruption.";

		Self::new(crate::ProblemCode::ResourceBusy, message).remedy(remedy)
	}

	/// Build a diagnostic from a filesystem error.
	pub fn io(context: impl std::fmt::Display, error: std::io::Error) -> Self {
		Self::new(crate::ProblemCode::IoError, format!("{context}: {error}"))
	}
}

impl std::fmt::Display for Problem {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		write!(f, "{}: {}", self.code, self.message)
	}
}
impl std::error::Error for Problem {}
