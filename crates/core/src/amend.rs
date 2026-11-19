//! Amend existing membership without treating ordinary invocation failure as task damage.
mod addition;
mod admission;
mod completion;
mod execution;
mod existing;
mod movement;
mod plan;
mod removal;
mod replacement;

use crate::{
	AmendGit, AmendRequest, AmendResult, AmendState, AmendmentChange, Control, Operation,
	OperationKind, OperationOutcome, Problem, ProblemCode, RemovalObserver, Task,
};

struct Amendment<'a, G, S: AmendState, O> {
	git: &'a G,
	store: &'a S,
	guard: &'a S::Admission,
	control: &'a Control,
	observer: &'a O,
	request: &'a AmendRequest,
	accept_pull: &'a [String],
	targets: &'a [crate::RemovalTarget],
	checkpoint: Option<S::Checkpoint>,
	changed: bool,
	uncertain: bool,
	changes: Vec<AmendmentChange>,
	pull_failures: Vec<crate::PullFailure>,
}

pub async fn amend(
	request: AmendRequest,
	git: &impl AmendGit,
	control: &Control,
	observer: &impl RemovalObserver,
	store: &impl AmendState,
) -> Result<AmendResult, Problem> {
	run(request, &[], false, git, control, observer, store).await
}

pub async fn recover_amend(
	request: AmendRequest,
	accept_pull: &[String],
	git: &impl AmendGit,
	control: &Control,
	observer: &impl RemovalObserver,
	store: &impl AmendState,
) -> Result<AmendResult, Problem> {
	run(request, accept_pull, true, git, control, observer, store).await
}

async fn run(
	mut request: AmendRequest,
	accept_pull: &[String],
	recovering: bool,
	git: &impl AmendGit,
	control: &Control,
	observer: &impl RemovalObserver,
	store: &impl AmendState,
) -> Result<AmendResult, Problem> {
	crate::validate_task(&request.task)?;
	if (request.repositories.is_empty() && request.remove.is_empty())
		|| (request.options.no_fetch && request.options.pull)
	{
		return Err(Problem::new(
			ProblemCode::InvalidInput,
			"Supply a repository or participant removal. --no-fetch and --pull are mutually exclusive.",
		));
	}
	control.check()?;
	let guard = store.admit(&request.task)?;
	let record = store.load(&guard, &request.task)?;
	admission::validate(&record, accept_pull)?;

	let restore_record = record.recorded.is_none()
		|| record.needs_publication
		|| record.task.operation.outcome != OperationOutcome::Succeeded;
	let original = record.into_amendment_task();
	if recovering {
		let options = request.removal_options;
		request = store.amend_request(&guard)?.ok_or_else(|| {
			Problem::new(
				ProblemCode::InvalidInput,
				"This task has no amendment to resume.",
			)
		})?;
		request.removal_options = options;
		admission::refresh_request(&mut request, &original, git, control).await?;
	}

	let plan = plan::plan(&request, &original, recovering, git, store, &guard, control).await?;
	// Persist resolved native locators and effective names so recovery is independent of CWD.
	if !recovering {
		request.repositories = plan
			.changes
			.iter()
			.map(|change| crate::RepositoryInput {
				registration_roots: change.participant.registration_roots.clone(),
				expected_identity: Some(change.repository.identity.clone()),
				path: change.repository.source.clone(),
				name: Some(change.name.clone()),
				remote: change.repository.remote.clone(),
				from: change.participant.from.clone(),
			})
			.collect();
	}

	let mut task = original.clone();
	task.removal = None;
	task.operation = Operation {
		kind: Some(OperationKind::Amend),
		continues: None,
		outcome: OperationOutcome::Pending,
	};
	let mut run = Amendment {
		git,
		store,
		guard: &guard,
		control,
		observer,
		request: &request,
		accept_pull,
		targets: &plan.targets,
		checkpoint: None,
		changed: false,
		uncertain: false,
		changes: Vec::new(),
		pull_failures: Vec::new(),
	};
	let result = run
		.execute(&mut task, plan.changes, recovering && restore_record)
		.await;
	if result.is_ok() && recovering && restore_record {
		run.begin(&task)?;
		run.changed = true;
		run.publish(&task, 0)?;
	}
	run.finish(task, original, result, recovering)
}

impl<G: AmendGit, S: AmendState, O: RemovalObserver> Amendment<'_, G, S, O> {
	fn begin(&mut self, task: &Task) -> Result<(), Problem> {
		self.control.check()?;
		self.store.verify()?;
		if self.checkpoint.is_none() {
			let checkpoint =
				self.store
					.begin_amend(self.guard, task, self.request, self.targets)?;
			self.checkpoint = Some(checkpoint);
		}
		Ok(())
	}

	fn publish(&self, task: &Task, index: usize) -> Result<(), Problem> {
		self.store
			.record_effect(self.guard, task, index, crate::EffectState::Settled)?;
		self.store.save(self.guard, task)
	}

	async fn report(
		&mut self,
		task: &Task,
		index: usize,
		kind: crate::AmendmentChangeKind,
	) -> Result<(), Problem> {
		self.report_named(task, &task.participants[index].name, kind)
			.await
	}

	async fn report_named(
		&mut self,
		task: &Task,
		participant: &str,
		kind: crate::AmendmentChangeKind,
	) -> Result<(), Problem> {
		self.changes.push(AmendmentChange {
			participant: participant.into(),
			kind,
		});
		self.observer
			.phase(&task.name, participant, kind.as_str())
			.await
	}
}
