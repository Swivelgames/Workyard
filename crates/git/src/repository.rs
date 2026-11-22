mod effects;
mod inspection;
mod planning;
pub(crate) mod remote;

use std::path::PathBuf;
use workyard_core::Problem;

/// Identity, location and selected remote of a backing Git repository.
#[derive(Clone, Debug)]
pub struct Repository {
	/// Path to the backing repository.
	pub source: PathBuf,
	/// Git administration directory shared by the repository's worktrees.
	pub common_dir: PathBuf,
	/// Remote selected for branch resolution or fetch.
	pub remote: Option<String>,
	/// Stable identifier used to match repository observations.
	pub identity: String,
}

fn utf8(bytes: &[u8]) -> Result<&str, Problem> {
	std::str::from_utf8(bytes).map_err(|_| {
		Problem::new(
			workyard_core::ProblemCode::InvalidGitOutput,
			"Git returned non-Unicode ref metadata.",
		)
	})
}

fn decode_path(bytes: &[u8]) -> Result<PathBuf, Problem> {
	crate::output::path_line(bytes).map_err(|_| {
		Problem::new(
			workyard_core::ProblemCode::InvalidGitOutput,
			"Git returned non-Unicode ref metadata.",
		)
	})
}

impl From<Repository> for workyard_core::Repository {
	fn from(repository: Repository) -> Self {
		Self {
			source: repository.source,
			common_dir: repository.common_dir,
			remote: repository.remote,
			identity: repository.identity,
		}
	}
}

impl From<&workyard_core::Repository> for Repository {
	fn from(repository: &workyard_core::Repository) -> Self {
		Self {
			source: repository.source.clone(),
			common_dir: repository.common_dir.clone(),
			remote: repository.remote.clone(),
			identity: repository.identity.clone(),
		}
	}
}
