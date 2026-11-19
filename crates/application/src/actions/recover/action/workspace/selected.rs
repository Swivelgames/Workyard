//! Recheck the selected task and Git placement before updating their locations.
use std::collections::HashSet;
use workyard_core::{
	Control, Participant, Problem, ProblemCode, RecoveryState, RemovalPhase, TaskLifecycle,
};
use workyard_git::{Placement, Runner};
use workyard_state::Store;

pub(super) struct Selected {
	task: String,
	generation: u64,
	participant: Participant,
	placement: Placement,
	pub row: usize,
}

impl Selected {
	pub fn new(
		task: &str,
		generation: u64,
		participant: Participant,
		placement: Placement,
		row: usize,
	) -> Self {
		Self {
			task: task.to_owned(),
			generation,
			participant,
			placement,
			row,
		}
	}

	pub fn needs_repair(&self) -> bool {
		self.placement.needs_repair
	}

	pub async fn apply(
		&self,
		store: &Store,
		runner: &Runner,
		control: &Control,
		activate: bool,
		unavailable_worktrees: &HashSet<String>,
	) -> Result<bool, Problem> {
		control.check()?;
		let guard = store.admit(&self.task)?;
		let record = store.load(&guard, &self.task)?;
		let published = record.recorded.as_ref().ok_or_else(|| {
			Problem::new(
				ProblemCode::DamagedState,
				"Task publication is unavailable.",
			)
		})?;
		// A revision can be durable before its publication. Match the same
		// participant in either document so replay can finish that last write.
		let index = record.task.participants.iter().position(|participant| {
			participant.name == self.participant.name
				&& participant.worktree_identity == self.participant.worktree_identity
				&& participant.backing_identity == self.participant.backing_identity
		});
		let Some(index) = index else {
			return Err(stale());
		};
		let current = &record.task.participants[index];
		let published_matches = published
			.participants
			.iter()
			.any(|p| p == &self.participant);
		if record.task.generation != self.generation
			|| record.task.lifecycle == TaskLifecycle::Retired
			|| record.pending == Some(index)
			|| record
				.task
				.removal
				.as_ref()
				.is_some_and(|removal| removal.participants[index].phase == RemovalPhase::Removed)
			|| !(current == &self.participant || published_matches)
		{
			return Err(stale());
		}

		let mut current = self.participant.clone();
		let workspace = record.task.state_dir.parent().expect("workspace marker");
		self.placement.update_hint(&mut current, workspace);
		if self.placement.needs_repair || current != self.participant {
			runner
				.apply_placement(
					&self.participant,
					&self.placement,
					activate,
					unavailable_worktrees,
					control,
				)
				.await?;
		}
		let remembered = store.remember_participant(&guard, &current)?;
		Ok(remembered || self.placement.needs_repair)
	}
}

fn stale() -> Problem {
	Problem::new(
		ProblemCode::RepositoryChanged,
		"Task changed since inspection. Run workyard recover again.",
	)
}
