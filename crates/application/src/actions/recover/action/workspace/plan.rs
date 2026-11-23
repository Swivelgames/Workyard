use super::super::super::workspace::{PlacementReport, PlacementState, WorkspaceRecovery};
use super::{inspect, selected::Selected};
use std::collections::HashSet;
use workyard_core::{Control, Participant, Problem, ProblemCode, RemovalPhase, TaskLifecycle};
use workyard_git::{Placement, Runner};
use workyard_state::{Reader, Store};

#[derive(Default)]
pub(super) struct Plan {
	selected: Vec<Selected>,
	unavailable_worktrees: HashSet<String>,
}

impl Plan {
	pub async fn inspect(
		reader: &Reader,
		runner: &Runner,
		control: &Control,
		report: &mut WorkspaceRecovery,
	) -> Result<Self, Problem> {
		let mut plan = Self::default();
		let mut projects = None;

		for snapshot in reader.tasks(None)? {
			control.check()?;

			if let Some(problem) = snapshot.problems.into_iter().next() {
				if let Some(task) = snapshot.task.or(snapshot.evidence) {
					plan.unavailable_worktrees.extend(
						task.participants
							.into_iter()
							.filter_map(|p| p.worktree_identity),
					);
				}
				report
					.participants
					.push(PlacementReport::unresolved(snapshot.name, None, problem));
				continue;
			}

			let Some(task) = snapshot.task else { continue };
			if task.lifecycle == TaskLifecycle::Retired {
				continue;
			}

			if !snapshot.busy
				&& snapshot.pending.is_none()
				&& task.removal.as_ref().is_some_and(|removal| {
					removal
						.participants
						.iter()
						.any(|participant| participant.phase == RemovalPhase::Removed)
				}) {
				report.interrupted_removals.push(task.name.clone());
			}

			for (index, participant) in task.participants.into_iter().enumerate() {
				let removed = task.removal.as_ref().is_some_and(|removal| {
					removal.participants[index].phase == RemovalPhase::Removed
				});
				if removed {
					continue;
				}

				let result = if snapshot.busy || snapshot.pending == Some(index) {
					Err(Problem::new(
						ProblemCode::ResourceBusy,
						"Task or participant has an active or unconfirmed operation.",
					))
				} else {
					inspect::participant(&participant, reader, runner, control, &mut projects).await
				};
				plan.record(&task.name, task.generation, participant, result, report);
			}
		}

		Ok(plan)
	}

	fn record(
		&mut self,
		task: &str,
		generation: u64,
		participant: Participant,
		result: Result<Placement, Problem>,
		report: &mut WorkspaceRecovery,
	) {
		let row = match result {
			Ok(placement) => {
				let row = PlacementReport::from_placement(task, &participant.name, &placement);
				self.selected.push(Selected::new(
					task,
					generation,
					participant,
					placement,
					report.participants.len(),
				));
				row
			}
			Err(problem) => {
				if let Some(id) = participant.worktree_identity {
					self.unavailable_worktrees.insert(id);
				}
				PlacementReport::unresolved(Some(task.to_owned()), Some(participant.name), problem)
			}
		};
		report.participants.push(row);
	}

	pub async fn apply(
		self,
		store: &Store,
		runner: &Runner,
		control: &Control,
		activate: bool,
		report: &mut WorkspaceRecovery,
	) -> bool {
		let mut changed = false;

		for selected in self.selected {
			let row = &mut report.participants[selected.row];
			match selected
				.apply(
					store,
					runner,
					control,
					activate,
					&self.unavailable_worktrees,
				)
				.await
			{
				Ok(true) => {
					changed = true;
					row.state = if selected.needs_repair() {
						PlacementState::Repaired
					} else {
						PlacementState::Current
					};
				}
				Ok(false) => row.state = PlacementState::Current,
				Err(problem) => {
					row.problem = Some(problem.into());
					row.state = PlacementState::Unresolved;
				}
			}
		}

		changed
	}
}
