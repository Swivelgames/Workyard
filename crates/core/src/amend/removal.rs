//! A name selects an incumbent once. Every later inspection and effect follows
//! that incumbent's backing and administrative identities.
use super::Amendment;
use crate::{
	AmendGit, AmendState, AmendmentChange, AmendmentChangeKind, EffectState, Problem, ProblemCode,
	RemovalObservation, RemovalObserver, RemovalTarget, Task, remove::consent,
};

impl<G: AmendGit, S: AmendState, O: RemovalObserver> Amendment<'_, G, S, O> {
	pub(super) async fn inspect_removals(
		&mut self,
		task: &Task,
		report_settled: bool,
	) -> Result<Vec<(RemovalTarget, RemovalObservation)>, Problem> {
		self.observer.begin_disclosure(task, Some(self.request));
		let mut selected = Vec::new();
		for target in self.targets {
			let Some(participant) = task.participants.iter().find(|p| target.matches(p)) else {
				if report_settled {
					self.changes.push(removed(&target.name));
				}
				continue;
			};
			self.control.check()?;
			let observation = self.git.inspect_removal(participant, self.control).await?;
			if observation.administration_identity != target.worktree_identity
				|| observation.repository.identity != target.backing_identity
			{
				return Err(Problem::new(
					ProblemCode::RepositoryChanged,
					"The selected worktree identity changed before removal.",
				));
			}
			self.observer.disclose(&target.name, &observation).await?;
			selected.push((target.clone(), observation));
		}

		if !selected.is_empty() {
			self.protect_membership(task, &selected).await?;
		}
		if !selected.is_empty() || self.observer.requires_unchanged_disclosure() {
			let names: Vec<_> = selected.iter().map(|(t, _)| t.name.clone()).collect();
			let dirty = selected.iter().any(|(_, o)| !o.dirty_paths.is_empty());
			consent::authorize(
				self.observer,
				self.request.removal_options,
				&task.name,
				Some(&names),
				dirty,
			)
			.await?;
		}
		Ok(selected)
	}

	/// A sibling can be moved with Git after the task record was saved. Find its
	/// current directory before asking the user to authorize a discard.
	async fn protect_membership(
		&self,
		task: &Task,
		selected: &[(RemovalTarget, RemovalObservation)],
	) -> Result<(), Problem> {
		for member in &task.participants {
			let selected_observation = selected.iter().find(|(target, _)| target.matches(member));
			let (location, backing) = match selected_observation {
				Some((_, observation)) => (
					observation.destination.clone(),
					Some(observation.repository.common_dir.clone()),
				),
				None => {
					let facts = self.git.observe(member, self.control).await?;
					(facts.path, facts.common_dir)
				}
			};
			for (target, observation) in selected {
				let tree = observation
					.destination
					.as_deref()
					.expect("verified removal location");
				let contains_tree = !target.matches(member)
					&& location.as_ref().is_some_and(|p| p.starts_with(tree));
				let contains_backing = backing
					.as_ref()
					.unwrap_or(&member.common_dir)
					.starts_with(tree);
				if contains_tree || contains_backing {
					return Err(Problem::new(ProblemCode::PathOverlap, format!(
						"Cannot unlink {:?}: its worktree contains a managed worktree or backing repository for {:?}.",
						target.name, member.name
					)).remedy("Move the nested worktree or repository outside the selected worktree and retry."));
				}
			}
		}
		Ok(())
	}

	pub(super) async fn unlink(
		&mut self,
		task: &mut Task,
		target: &RemovalTarget,
		observation: RemovalObservation,
	) -> Result<(), Problem> {
		let index = task
			.participants
			.iter()
			.position(|p| target.matches(p))
			.expect("admitted removal target");
		let participant = &task.participants[index];
		let mut disclosed = observation;
		match consent::unlink(
			self.git,
			self.observer,
			self.control,
			participant,
			&mut disclosed,
			|_| {
				self.begin(task)?;
				self.store
					.record_effect(self.guard, task, index, EffectState::Pending)
			},
		)
		.await
		{
			Ok(()) => {}
			Err(crate::UnlinkFailure::BeforeLaunch(problem)) => return Err(problem),
			Err(crate::UnlinkFailure::Command(problem)) => {
				self.settle_failure(task, index, &disclosed, &problem)
					.await?;
				return Err(problem);
			}
		}

		self.changed = true;
		task.participants.remove(index);
		self.changes.push(removed(&target.name));
		if let Err(problem) = self.store.settle_unlink(self.guard, task, target) {
			self.uncertain = true;
			return Err(problem);
		}
		self.store.save(self.guard, task)?;
		self.observer.removed(&task.name, &target.name).await
	}

	async fn settle_failure(
		&mut self,
		task: &Task,
		index: usize,
		before: &RemovalObservation,
		problem: &Problem,
	) -> Result<(), Problem> {
		if problem.code == ProblemCode::LaunchFailed {
			return self
				.store
				.record_effect(self.guard, task, index, EffectState::Settled);
		}
		if problem.code == ProblemCode::SupervisionFailed {
			self.uncertain = true;
			return Ok(());
		}

		// A supervised nonzero exit can still have changed files. Clear the
		// writer only after finding the same worktree; keep partial effects.
		let settled = crate::Control::new(|| false);
		let observed = self
			.git
			.inspect_removal(&task.participants[index], &settled)
			.await;
		match observed {
			Ok(after) if before.same_worktree(&after) => {
				self.changed |= before.dirty_token != after.dirty_token;
				self.store
					.record_effect(self.guard, task, index, EffectState::Settled)
			}
			_ => {
				self.uncertain = true;
				Ok(())
			}
		}
	}
}

fn removed(name: &str) -> AmendmentChange {
	AmendmentChange {
		participant: name.into(),
		kind: AmendmentChangeKind::Removed,
	}
}
