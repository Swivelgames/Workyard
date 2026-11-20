//! Select a named recovery operation from its current evidence and the caller's authority.
use workyard_core::{
	AmendRecovery, AmendState, OperationKind, Problem, ProblemCode, RecoveryScope, RecoveryState,
	RemovalOptions, Task,
};
use workyard_state::Store;

pub(super) enum Authority {
	General(RemovalOptions),
	Participants,
}

pub(super) enum Route {
	Creation(RecoveryScope),
	Amendment(AmendRecovery),
	Removal(RemovalOptions),
}

impl Route {
	pub(super) fn select(name: &str, authority: Authority, store: &Store) -> Result<Self, Problem> {
		let guard = store.admit(name)?;
		let task = store.load(&guard, name)?.task;
		let options = match authority {
			Authority::Participants => return Self::participants(&task),
			Authority::General(options) => options,
		};

		if task.removal.is_some() {
			return Ok(Self::Removal(options));
		}
		if options.undo {
			return Err(Problem::new(
				ProblemCode::InvalidInput,
				"--undo applies only to whole-task removal recovery.",
			));
		}

		if task.operation.is_continuing(OperationKind::Amend)
			&& let Some(request) = store.amend_request(&guard)?
		{
			let removal = RemovalOptions {
				yes: options.yes,
				force: options.force,
				..request.removal_options
			};
			return Ok(Self::Amendment(AmendRecovery::WithRemovalOptions(removal)));
		}
		if options.force || options.yes {
			return Err(Problem::new(
				ProblemCode::InvalidInput,
				"--yes and --force require a pending removal.",
			));
		}
		Ok(Self::Creation(RecoveryScope::Reconcile))
	}

	fn participants(task: &Task) -> Result<Self, Problem> {
		if task.removal.is_some() {
			let problem = Problem::new(
				ProblemCode::InvalidInput,
				"Named participant recovery cannot resume whole-task removal.",
			);
			return Err(problem.remedy(
				"Inspect the task with status. Use CLI or RPC recovery for whole-task removal.",
			));
		}
		if task.operation.is_continuing(OperationKind::Amend) {
			return Ok(Self::Amendment(AmendRecovery::AddOrRename));
		}
		Ok(Self::Creation(RecoveryScope::CreateOnly))
	}
}
