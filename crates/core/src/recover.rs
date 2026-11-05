use crate::{
	Control, Observer, Phase, Problem, ProblemCode, RecoveryGit, RecoveryState, SourceResolution,
	Task, TaskStatus,
};
use std::path::PathBuf;

struct Recovery<'a, G, S: RecoveryState, O> {
	runner: &'a G,
	control: &'a Control,
	observer: &'a O,
	store: &'a S,
	guard: &'a S::Admission,
}

pub async fn recover(
	name: &str,
	accept_pull: &[String],
	runner: &impl RecoveryGit,
	control: &Control,
	observer: &impl Observer,
	store: &impl RecoveryState,
) -> Result<(Task, PathBuf), Problem> {
	crate::validate_task(name)?;
	control.check()?;
	let guard = store.admit(name)?;
	let record = store.load(&guard, name)?;
	let mut task = prepare_recovery(record.task);

	for accepted in accept_pull {
		if !task
			.participants
			.iter()
			.any(|p| &p.name == accepted && p.phase == Phase::Pulling)
		{
			return Err(Problem::new(
				ProblemCode::InvalidInput,
				format!(
					"--accept-pull {accepted:?} must name a participant with an unsettled pull outcome."
				),
			));
		}
	}

	if let Some(index) = record.pending
		&& task.participants[index].phase != Phase::Fetching
	{
		let participant = &task.participants[index].name;
		let message = format!(
			"Participant {participant:?} has no durable proof its managed Git writers stopped."
		);
		return Err(Problem::new(ProblemCode::ResourceBusy, message)
				.remedy("Do not retry or accept the effect based on a free task lock. Inspect the interrupted Workyard process and Git process tree. This release has no force or writer-quarantine override: preserve state, continue manual Git repair as needed, and restore verified settled evidence from a backup only if available. --accept-pull cannot bypass this check."));
	}

	let path = store.record_path(&guard);
	let recovery = Recovery {
		runner,
		control,
		observer,
		store,
		guard: &guard,
	};
	let result = recovery.resume(&mut task, accept_pull).await;

	task.status = match &result {
		Ok(()) => TaskStatus::Completed,
		Err(problem) if problem.code == ProblemCode::Cancelled => TaskStatus::Cancelled,
		Err(_) => TaskStatus::Failed,
	};
	task.problem = result.err();
	if let Err(problem) = store.save(&guard, &task) {
		task.status = TaskStatus::Failed;
		task.problem = Some(problem);
	}
	Ok((task, path))
}

impl<G: RecoveryGit, S: RecoveryState, O: Observer> Recovery<'_, G, S, O> {
	async fn resume(&self, task: &mut Task, accepted: &[String]) -> Result<(), Problem> {
		for index in 0..task.participants.len() {
			self.control.check()?;
			self.store.verify()?;
			self.participant(task, index, accepted).await?;
		}
		Ok(())
	}

	async fn participant(
		&self,
		task: &mut Task,
		index: usize,
		accepted: &[String],
	) -> Result<(), Problem> {
		let observed = self
			.runner
			.reconcile(&task.participants[index], &task.name, self.control)
			.await?;
		if observed.operation_in_progress {
			let name = &task.participants[index].name;
			let message = format!(
				"Participant {name:?} has native Git locks or an unfinished Git operation."
			);
			return Err(Problem::new(ProblemCode::ResourceBusy, message)
				.remedy("Inspect git status, finish or abort the Git operation manually, and wait for native writers to stop. Workyard never removes Git locks."));
		}

		let repository = observed.repository;
		let previous = task.participants[index].clone();
		let participant = &mut task.participants[index];
		participant.source = repository.source.clone();
		participant.common_dir = repository.common_dir.clone();

		if let Some(destination) = observed.destination {
			participant.destination = destination;
			if participant.phase == Phase::Completed {
				let cache_changed = participant.source != previous.source
					|| participant.common_dir != previous.common_dir
					|| participant.destination != previous.destination;
				if cache_changed {
					self.store.record_effect(
						self.guard,
						task,
						index,
						crate::EffectState::Settled,
					)?;
				}
				return Ok(());
			}
			if participant.phase == Phase::Pulling {
				if !accepted.contains(&participant.name) {
					let message = format!(
						"Pull outcome for {:?} needs explicit acceptance after manual repair.",
						participant.name
					);
					let remedy = format!(
						"Inspect git status and complete or abort the pull yourself. Then run recover {:?} --state-dir {:?} --accept-pull {:?}. No pull will be replayed.",
						task.name, task.state_dir, participant.name
					);
					return Err(Problem::new(ProblemCode::GitFailed, message).remedy(remedy));
				}
				return self.phase(task, index, Phase::Completed).await;
			}
			participant.phase = Phase::WorktreeCreated;
		} else {
			self.create_missing(task, index, &repository).await?;
		}

		if task.pull {
			self.pull(task, index, &repository).await?;
		}
		self.phase(task, index, Phase::Completed).await
	}

	async fn create_missing(
		&self,
		task: &mut Task,
		index: usize,
		repository: &crate::Repository,
	) -> Result<(), Problem> {
		let participant = &task.participants[index];
		if matches!(
			participant.phase,
			Phase::WorktreeCreated | Phase::Pulling | Phase::Completed
		) {
			return Err(Problem::new(
				ProblemCode::RepositoryChanged,
				"Previously created worktree is missing; recovery will not recreate authored work.",
			)
			.remedy("Restore access or repair Git worktree relationships, then recover again."));
		}
		if participant.destination != task.root.join(&task.name).join(&participant.name) {
			return Err(Problem::new(
				ProblemCode::DamagedState,
				"Unstarted destination disagrees with admitted task placement.",
			));
		}

		let fetch_needed = !task.no_fetch
			&& repository.remote.is_some()
			&& matches!(participant.phase, Phase::NotStarted | Phase::Fetching);
		if fetch_needed {
			self.phase(task, index, Phase::Fetching).await?;
			self.store
				.record_effect(self.guard, task, index, crate::EffectState::Pending)?;
			let result = self.runner.fetch(repository, self.control).await;
			if result.is_ok() {
				task.participants[index].phase = Phase::Creating;
			}
			self.store
				.record_effect(self.guard, task, index, crate::EffectState::Settled)?;
			result?;
		}

		self.phase(task, index, Phase::Creating).await?;
		self.store.verify()?;
		self.runner.revalidate(repository, self.control).await?;
		let participant = &task.participants[index];
		let plan = self
			.runner
			.plan(
				repository,
				&task.name,
				task.checkout,
				participant.from.as_deref(),
				SourceResolution::Cached,
				self.control,
			)
			.await?;
		self.store.validate_destination(&participant.destination)?;
		self.store
			.record_effect(self.guard, task, index, crate::EffectState::Pending)?;
		let result = self
			.runner
			.add(
				repository,
				&task.name,
				&participant.destination,
				&plan,
				self.control,
			)
			.await;
		crate::settle_add::settle_add(result, task, index, self.runner, self.store, self.guard)
			.await
	}

	async fn pull(
		&self,
		task: &mut Task,
		index: usize,
		repository: &crate::Repository,
	) -> Result<(), Problem> {
		self.phase(task, index, Phase::Pulling).await?;
		self.runner.revalidate(repository, self.control).await?;
		let participant = &task.participants[index];
		let current = self
			.runner
			.reconcile(participant, &task.name, self.control)
			.await?;
		if current.operation_in_progress
			|| current.destination.as_ref() != Some(&participant.destination)
		{
			return Err(Problem::new(
				ProblemCode::RepositoryChanged,
				"Participant changed while recovery progress was delivered.",
			)
			.remedy(
				"Inspect the participant Git operation and worktree location, then retry recovery.",
			));
		}
		self.store
			.record_effect(self.guard, task, index, crate::EffectState::Pending)?;
		let result = self
			.runner
			.pull(repository, &participant.destination, self.control)
			.await;
		crate::publication::settle_pull(result, task, index, self.store, self.guard)
	}

	async fn phase(&self, task: &mut Task, index: usize, phase: Phase) -> Result<(), Problem> {
		// Reconciled observations are published before cancellation is returned.
		crate::publication::phase(task, index, phase, self.store, self.guard, self.observer)
			.await?;
		self.control.check()
	}
}

/// Recovery starts a new attempt without changing the observed participant facts.
fn prepare_recovery(mut task: Task) -> Task {
	task.status = TaskStatus::Pending;
	task.problem = None;
	task
}
