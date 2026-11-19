use std::path::PathBuf;

#[derive(Clone, Debug)]
pub enum ProjectRemedy {
	Choose(Vec<PathBuf>),
	Rename { source: PathBuf, name: String },
}

#[derive(Clone, Debug)]
pub struct Problem {
	pub code: crate::ProblemCode,
	pub message: String,
	pub remedy: String,
	pub project_remedy: Option<Box<ProjectRemedy>>,
}

impl Problem {
	pub fn new(code: crate::ProblemCode, message: impl Into<String>) -> Self {
		Self {
			code,
			message: message.into(),
			remedy: String::new(),
			project_remedy: None,
		}
	}

	pub fn remedy(mut self, remedy: impl Into<String>) -> Self {
		self.remedy = remedy.into();
		self
	}

	/// A later invocation cannot infer process exit from an available Workyard lock.
	pub(crate) fn unconfirmed_git_exit(context: impl std::fmt::Display) -> Self {
		let message = format!("Workyard cannot confirm Git finished {context}.");
		let remedy = "You can inspect files. Leave the worktree in place while checking Git and its hooks. \
			If the original Workyard process has exited, retrying cannot clear this interruption.";

		Self::new(crate::ProblemCode::ResourceBusy, message).remedy(remedy)
	}

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
