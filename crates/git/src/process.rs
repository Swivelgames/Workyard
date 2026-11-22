mod capture;
mod supervision;

use std::{
	ffi::{OsStr, OsString},
	path::{Path, PathBuf},
	process::ExitStatus,
};
use supervision::supervise;
use workyard_core::{Control, Problem};

/// Exit status and bounded captured streams from a managed Git subprocess.
pub struct CommandOutput {
	/// Exit status returned by Git.
	pub status: ExitStatus,
	/// Captured standard output bytes.
	pub stdout: Vec<u8>,
	/// Captured diagnostic bytes.
	pub stderr: Vec<u8>,
	/// Whether output exceeded the capture budget.
	pub truncated: bool,
	/// Whether supervision interrupted the command.
	pub interrupted: bool,
}

impl CommandOutput {
	/// Build a diagnostic from the exit status, captured stderr and invocation.
	pub fn problem(&self, directory: &Path, arguments: &[OsString]) -> Problem {
		let diagnostic = String::from_utf8_lossy(&self.stderr);
		let truncation = if self.truncated {
			" [output truncated]"
		} else {
			""
		};
		let message = format!(
			"git {arguments:?} in {directory:?} exited {}: {diagnostic}{truncation}",
			self.status
		);
		let remedy = format!(
			"Inspect `git -C {directory:?} status` and \
			`git -C {directory:?} worktree list --porcelain`. \
			Preserve successful worktrees and resolve the reported Git problem."
		);
		Problem::new(workyard_core::ProblemCode::GitFailed, message).remedy(remedy)
	}
}

/// Launch and supervise Git commands with bounded output capture.
#[derive(Clone)]
pub struct Runner {
	executable: PathBuf,
}

impl Default for Runner {
	fn default() -> Self {
		Self::new("git")
	}
}

impl Runner {
	/// Select the Git executable to run.
	pub fn new(executable: impl Into<PathBuf>) -> Self {
		Self {
			executable: executable.into(),
		}
	}

	/// Execute native arguments in the supplied directory under the cancellation policy.
	pub async fn run(
		&self,
		directory: &Path,
		arguments: &[OsString],
		control: &Control,
	) -> Result<CommandOutput, Problem> {
		control.check()?;
		let executable = self.executable.clone();
		let directory = directory.to_path_buf();
		let arguments = arguments.to_vec();
		let control = control.clone();
		smol::unblock(move || supervise(&executable, &directory, &arguments, &control)).await
	}

	/// Execute Git with UTF-8 arguments.
	pub async fn git(
		&self,
		directory: &Path,
		arguments: &[&str],
		control: &Control,
	) -> Result<CommandOutput, Problem> {
		let arguments = args(arguments);
		self.run(directory, &arguments, control).await
	}

	pub(crate) async fn effect(
		&self,
		directory: &Path,
		arguments: &[OsString],
		control: &Control,
	) -> Result<(), Problem> {
		let output = self.run(directory, arguments, control).await?;
		if !output.status.success() {
			return Err(output.problem(directory, arguments));
		}
		// Truncated diagnostics must not discard an observed successful effect.
		Ok(())
	}

	/// Return stdout after checking process status and capture completeness.
	pub async fn checked(
		&self,
		directory: &Path,
		arguments: &[OsString],
		control: &Control,
	) -> Result<Vec<u8>, Problem> {
		let output = self.run(directory, arguments, control).await?;
		if !output.status.success() {
			return Err(output.problem(directory, arguments));
		}

		if output.truncated {
			return Err(Problem::new(
				workyard_core::ProblemCode::GitOutputLimit,
				"Git output exceeded the inspection limit.",
			));
		}

		Ok(output.stdout)
	}
}

pub(crate) fn args(items: &[impl AsRef<OsStr>]) -> Vec<OsString> {
	items
		.iter()
		.map(|item| item.as_ref().to_os_string())
		.collect()
}
