//! Amend existing membership without treating ordinary invocation failure as task damage.
mod addition;
mod admission;
mod completion;
mod execution;
mod existing;
mod intent;
mod movement;
mod plan;
mod removal;
mod replacement;

pub use intent::AmendRecovery;
use intent::Intent;

use crate::{
	AmendGit, AmendRequest, AmendResult, AmendState, AmendmentChange, Control, Operation,
	OperationKind, OperationOutcome, Problem, RemovalObserver, Task,
};

struct Amendment<'a, G, S: AmendState, O> {
	git: &'a G,
	store: &'a S,
	guard: &'a S::Admission,
	control: &'a Control,
	observer: &'a O,
	request: &'a AmendRequest,
	targets: &'a [crate::RemovalTarget],
	checkpoint: Option<S::Checkpoint>,
	changed: bool,
	uncertain: bool,
	changes: Vec<AmendmentChange>,
}

/// Apply the requested membership changes to a managed task.
pub async fn amend(
	request: AmendRequest,
	git: &impl AmendGit,
	control: &Control,
	observer: &impl RemovalObserver,
	store: &impl AmendState,
) -> Result<AmendResult, Problem> {
	admission::validate_selection(&request)?;
	let name = request.task.clone();
	run(&name, Intent::Apply(request), git, control, observer, store).await
}

/// Resume the recorded amendment using its saved intent.
pub async fn recover_amend(
	request: AmendRequest,
	git: &impl AmendGit,
	control: &Control,
	observer: &impl RemovalObserver,
	store: &impl AmendState,
) -> Result<AmendResult, Problem> {
	admission::validate_selection(&request)?;
	let scope = AmendRecovery::WithRemovalOptions(request.removal_options);
	recover_amend_with_scope(&request.task, scope, git, control, observer, store).await
}

/// Resume an amendment under the caller's recovery policy.
pub async fn recover_amend_with_scope(
	name: &str,
	scope: AmendRecovery,
	git: &impl AmendGit,
	control: &Control,
	observer: &impl RemovalObserver,
	store: &impl AmendState,
) -> Result<AmendResult, Problem> {
	run(name, Intent::Resume(scope), git, control, observer, store).await
}

async fn run(
	name: &str,
	intent: Intent,
	git: &impl AmendGit,
	control: &Control,
	observer: &impl RemovalObserver,
	store: &impl AmendState,
) -> Result<AmendResult, Problem> {
	crate::validate_task(name)?;
	control.check()?;
	let guard = store.admit(name)?;
	let record = store.load(&guard, name)?;
	let recovering = matches!(intent, Intent::Resume(_));
	admission::validate(&record)?;
	let mut request = intent.resolve(&record, store, &guard)?;

	let restore_record = recovering && completion::needs_publication(&record, &request);
	let original = record.into_amendment_task();
	if recovering {
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
		targets: &plan.targets,
		checkpoint: None,
		changed: false,
		uncertain: false,
		changes: Vec::new(),
	};
	let result = run.execute(&mut task, plan.changes, restore_record).await;
	if result.is_ok() && restore_record {
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
