//! Recheck the removal disclosure while holding the repository relationship lock.
use super::{blocked, dirty};
use crate::{Runner, process::args};
use std::path::Path;
use workyard_core::{Control, Problem, ProblemCode, RemovalObservation, RetainedHistory};

impl Runner {
	pub(super) async fn validate_disclosure(
		&self,
		observation: &RemovalObservation,
		destination: &Path,
		admin: &Path,
		control: &Control,
	) -> Result<(), Problem> {
		require_removable_admin(admin, &observation.repository.common_dir)?;
		let branch = self
			.checked(
				destination,
				&args(&["rev-parse", "--abbrev-ref", "HEAD"]),
				control,
			)
			.await?;
		let branch = std::str::from_utf8(&branch)
			.map_err(|_| blocked("Invalid Git branch output."))?
			.trim_end_matches(['\r', '\n']);
		let history = match observation.branch.as_deref() {
			Some(expected) if branch == expected => {
				self.retained_history(destination, branch, control).await?
			}
			None if branch == "HEAD" => {
				self.require_retained_head(destination, control).await?;
				RetainedHistory::Detached
			}
			_ => return Err(blocked("Worktree branch changed after disclosure.")),
		};
		let (_, token) =
			dirty::inspect(self, destination, observation.worktree_entry, control).await?;
		if token != observation.dirty_token || history != observation.history {
			return Err(Problem::new(
				ProblemCode::DecisionRequired,
				"The worktree disclosure changed before unlinking.",
			)
			.remedy("Retry the removal to review the newly changed files."));
		}
		Ok(())
	}

	pub(super) async fn require_retained_head(
		&self,
		destination: &Path,
		control: &Control,
	) -> Result<(), Problem> {
		let references = self
			.checked(
				destination,
				&args(&[
					"for-each-ref",
					"--contains=HEAD",
					"--format=%(refname)",
					"refs/heads",
					"refs/tags",
				]),
				control,
			)
			.await?;
		if references.is_empty() {
			return Err(blocked(
				"Detached HEAD is not reachable from a retained local branch or tag. Create a branch or tag before removal.",
			));
		}
		Ok(())
	}
}

pub(super) fn require_removable_admin(admin: &Path, common: &Path) -> Result<(), Problem> {
	// Git can retain submodule objects here after deinit. Forced removal also
	// deletes this private directory, including commits absent from any remote.
	let modules = admin.join("modules");
	match std::fs::symlink_metadata(&modules) {
		Ok(_) => {
			return Err(Problem::new(
				ProblemCode::PathOverlap,
				format!("This worktree retains submodule storage at {modules:?}."),
			)
			.remedy(
				"Deinitialize any checked-out submodules, then move the retained storage outside this worktree and its Git directory before retrying.",
			));
		}
		Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
		Err(error) => return Err(Problem::io("Inspect retained submodule storage", error)),
	}

	if crate::operations::operations_present(admin, common)?
		|| crate::operations::locks_present(admin, common)?
		|| admin
			.join("locked")
			.try_exists()
			.map_err(|e| Problem::io("Inspect worktree lock", e))?
	{
		return Err(blocked(
			"Worktree or backing repository has an active Git operation.",
		));
	}
	Ok(())
}
