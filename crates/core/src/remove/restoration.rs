mod receipt;
pub(super) use receipt::retain_recorded_identity;

use super::{RemovalRun, verification::participant as verify_participant};
use crate::{
	BranchPlan, Decision, DecisionKind, EffectState, Phase, Problem, ProblemCode, RecoveryState,
	RemovalDirection, RemovalGit, RemovalObserver, RemovalPhase, Task,
};

impl<G: RemovalGit, S: RecoveryState, O: RemovalObserver> RemovalRun<'_, G, S, O> {
	pub(super) async fn restore(&mut self, task: &mut Task) -> Result<(), Problem> {
		let removal = task.removal.as_ref().ok_or_else(|| {
			Problem::new(
				ProblemCode::InvalidInput,
				"This task has no removal to undo.",
			)
		})?;
		if removal
			.participants
			.iter()
			.any(|participant| participant.dirty_at_start)
			&& !self.options.yes
			&& !self.options.force
		{
			let decision = Decision {
				task: Some(task.name.clone()),
				kind: DecisionKind::LostChanges,
				participant: String::new(),
				participants: Vec::new(),
				message: "Recreating worktrees restores retained branch contents only. Discarded modified, untracked and ignored files cannot be restored. Continue?".into(),
			};
			super::consent::require(self.observer, decision).await?;
		}

		// Validate survivors as well as destinations before publishing or recreating anything.
		for index in 0..task.participants.len() {
			self.preflight_restoration(task, index).await?;
		}
		// Direction is durable before effects; repeating undo never toggles back.
		task.removal.as_mut().expect("removal intent").direction = RemovalDirection::Restore;
		self.publish(task, 0)?;

		for index in 0..task.participants.len() {
			self.control.check()?;
			let phase = task.removal.as_ref().expect("removal intent").participants[index].phase;
			if matches!(phase, RemovalPhase::Present | RemovalPhase::Restored) {
				verify_participant(&mut task.participants[index], self.git, self.control).await?;
				continue;
			}
			if phase == RemovalPhase::Recreated {
				self.reconcile_recreation(task, index).await?;
			} else {
				self.recreate(task, index).await?;
			}
			self.finish_recreation(task, index).await?;
		}
		// Earlier participants may change while later Git operations run.
		for participant in &mut task.participants {
			verify_participant(participant, self.git, self.control).await?;
		}
		Ok(())
	}

	async fn preflight_restoration(&self, task: &mut Task, index: usize) -> Result<(), Problem> {
		self.control.check()?;
		let phase = task.removal.as_ref().expect("removal intent").participants[index].phase;
		match phase {
			RemovalPhase::Present | RemovalPhase::Restored => {
				verify_participant(&mut task.participants[index], self.git, self.control).await
			}
			RemovalPhase::Recreated => self.reconcile_recreation(task, index).await,
			RemovalPhase::Removed | RemovalPhase::Restoring => {
				let branch = retained_branch(task, index)?;
				self.git
					.inspect_restoration(&task.participants[index], branch, self.control)
					.await?;
				Ok(())
			}
		}
	}

	async fn reconcile_recreation(&self, task: &mut Task, index: usize) -> Result<(), Problem> {
		let branch = retained_branch(task, index)?;
		let observed = self
			.git
			.reconcile_recreation(&task.participants[index], branch, self.control)
			.await?;
		let destination = observed.destination.ok_or_else(|| {
			Problem::new(
				ProblemCode::RepositoryChanged,
				"Recreated worktree could not be verified.",
			)
		})?;
		if observed.operation_in_progress {
			return Err(Problem::new(
				ProblemCode::ResourceBusy,
				"Recreated worktree has unfinished Git work.",
			));
		}
		let participant = &mut task.participants[index];
		participant.source = observed.repository.source;
		participant.common_dir = observed.repository.common_dir;
		participant.destination = destination;
		Ok(())
	}

	async fn recreate(&mut self, task: &mut Task, index: usize) -> Result<(), Problem> {
		let branch = retained_branch(task, index)?.to_owned();
		let repository = self
			.git
			.inspect_restoration(&task.participants[index], &branch, self.control)
			.await?;
		self.store.verify()?;
		task.removal.as_mut().expect("removal intent").participants[index].phase =
			RemovalPhase::Restoring;
		// The removed worktree's administrative identity cannot identify its replacement.
		task.participants[index].worktree_identity = None;
		task.participants[index].placement = None;
		self.store
			.record_effect(self.guard, task, index, EffectState::Pending)?;
		let result = self
			.git
			.add(
				&repository,
				&branch,
				&task.participants[index].destination,
				&BranchPlan::Reuse,
				self.control,
			)
			.await;
		if result
			.as_ref()
			.is_err_and(|problem| problem.code == ProblemCode::SupervisionFailed)
		{
			return result;
		}
		if result.is_ok() {
			task.removal.as_mut().expect("removal intent").participants[index].phase =
				RemovalPhase::Recreated;
		}
		// Persist successful add separately from writer settlement before secondary reads.
		self.publish(task, index)?;
		result
	}

	async fn finish_recreation(&mut self, task: &mut Task, index: usize) -> Result<(), Problem> {
		task.participants[index].worktree_identity = self
			.git
			.worktree_identity(&task.participants[index].destination)
			.await?;
		task.participants[index].phase = Phase::Completed;
		task.removal.as_mut().expect("removal intent").participants[index].phase =
			RemovalPhase::Restored;
		self.publish(task, index)?;
		self.observer
			.phase(&task.name, &task.participants[index].name, "restored")
			.await
	}
}

fn retained_branch(task: &Task, index: usize) -> Result<&str, Problem> {
	task.removal.as_ref().expect("removal intent").participants[index]
		.branch
		.as_deref()
		.ok_or_else(|| {
			Problem::new(
				ProblemCode::SourceMissing,
				"Detached worktree has no retained local branch for undo.",
			)
			.remedy(
				"Recreate it manually from its retained reference; undo does not invent a branch.",
			)
		})
}
