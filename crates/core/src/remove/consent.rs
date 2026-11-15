use super::RemovalRun;
use crate::{
	Decision, DecisionKind, Problem, ProblemCode, RecoveryState, RemovalGit, RemovalObservation,
	RemovalObserver,
};

impl<G: RemovalGit, S: RecoveryState, O: RemovalObserver> RemovalRun<'_, G, S, O> {
	pub(super) async fn authorize_task(
		&self,
		task: &str,
		observations: &[Option<RemovalObservation>],
	) -> Result<(), Problem> {
		let discards_files = observations
			.iter()
			.flatten()
			.any(|item| !item.dirty_paths.is_empty());
		if self.options.force || (self.options.yes && !discards_files) {
			return Ok(());
		}

		let decision = Decision {
			task: Some(task.into()),
			participant: String::new(),
			kind: if discards_files {
				DecisionKind::DiscardTask
			} else {
				DecisionKind::RemoveTask
			},
			message: if discards_files {
				"Remove the task and discard the disclosed files? Discarded changes cannot be restored.".into()
			} else {
				"Remove the task worktrees while retaining branches and local commits?".into()
			},
		};
		if discards_files {
			return self.require(decision).await;
		}

		// Noninteractive removal of clean trees needs no convenience confirmation.
		if self.observer.confirm(decision).await? == Some(false) {
			return Err(declined());
		}
		Ok(())
	}

	pub(super) async fn authorize_changed_files(&self, participant: &str) -> Result<(), Problem> {
		self.require(Decision {
			task: None,
			kind: DecisionKind::DiscardFiles,
			participant: participant.into(),
			message: "Discard the newly disclosed files? These changes cannot be restored by undo."
				.into(),
		})
		.await
	}

	pub(super) async fn require(&self, decision: Decision) -> Result<(), Problem> {
		let message = decision.message.clone();
		match self.observer.confirm(decision).await? {
			Some(true) => Ok(()),
			Some(false) => Err(declined()),
			None => Err(Problem::new(ProblemCode::DecisionRequired, message)
				.remedy("Review the disclosed facts. Use --force only to authorize file discard, or --yes to acknowledge an informational undo warning.")),
		}
	}
}

fn declined() -> Problem {
	Problem::new(
		ProblemCode::Cancelled,
		"Confirmation declined; no further worktrees will be changed.",
	)
}
