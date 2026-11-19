//! Recheck the removal disclosure while holding the repository relationship lock.
use super::{blocked, dirty};
use crate::{Runner, process::args};
use std::path::Path;
use workyard_core::{Control, Problem, ProblemCode, RemovalObservation};

impl Runner {
	pub(super) async fn validate_disclosure(
		&self,
		observation: &RemovalObservation,
		destination: &Path,
		admin: &Path,
		control: &Control,
	) -> Result<(), Problem> {
		require_idle(admin, &observation.repository.common_dir)?;
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
		match observation.branch.as_deref() {
			Some(expected) if branch == expected => {}
			None if branch == "HEAD" => self.require_retained_head(destination, control).await?,
			_ => return Err(blocked("Worktree branch changed after disclosure.")),
		}
		let (paths, token) = dirty::inspect(self, destination, control).await?;
		if !paths.is_empty() && token != observation.dirty_token {
			return Err(Problem::new(
				ProblemCode::DecisionRequired,
				"Worktree files changed before unlinking.",
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

pub(super) fn require_idle(admin: &Path, common: &Path) -> Result<(), Problem> {
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
