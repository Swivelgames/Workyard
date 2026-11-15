use trine::InvocationContext;
use workyard_core::{Control, OperationOutcome, ProblemCode, RecoveryState};
use workyard_git::Runner;
use workyard_state::Store;

use crate::{
	common::Progress,
	invocation::{Events, blocking_action},
};

use super::{RecoverInput, RecoverOutcome};

pub async fn recover(
	input: RecoverInput,
	context: InvocationContext<Progress>,
) -> Result<RecoverOutcome, RecoverOutcome> {
	blocking_action(run(input, context)).await
}

async fn run(
	input: RecoverInput,
	context: InvocationContext<Progress>,
) -> Result<RecoverOutcome, RecoverOutcome> {
	let name = input.task.clone();
	let rejected = |problem| RecoverOutcome::rejected(name.clone(), problem);
	workyard_core::validate_task(&name).map_err(rejected)?;

	let cancellation = context.cancellation().clone();
	let control = Control::new(move || cancellation.is_cancelled());
	control.check().map_err(rejected)?;

	let location = input.location().map_err(rejected)?;
	let store =
		Store::open_existing(&location.state, location.root.as_deref()).map_err(rejected)?;
	let runner = Runner::default();
	let accepted = input.accept_pull;
	let guard = store.admit(&name).map_err(rejected)?;
	let record = store.load(&guard, &name).map_err(rejected)?;
	let removing = record.task.removal.is_some();
	drop(guard);
	if removing {
		if !accepted.is_empty() {
			return Err(rejected(workyard_core::Problem::new(
				ProblemCode::InvalidInput,
				"--accept-pull does not apply to removal.",
			)));
		}
		let observer = crate::removal::RemovalEvents::new(context, "recover", name.clone());
		let options = workyard_core::RemovalOptions {
			undo: input.undo,
			yes: input.yes,
			force: input.force,
		};
		let result =
			workyard_core::recover_removal(&name, options, &runner, &control, &observer, &store)
				.await;
		let mut outcome = match result {
			Ok(result) => RecoverOutcome::from_removal(result),
			Err(problem) => rejected(problem),
		};
		outcome.disclosures = observer.disclosures.into_inner().expect("disclosures");
		outcome.decision = observer.decision.into_inner().expect("decision");
		return if outcome.operation.outcome == OperationOutcome::Failed {
			Err(outcome)
		} else {
			Ok(outcome)
		};
	}
	if input.undo || input.force || input.yes {
		return Err(rejected(workyard_core::Problem::new(
			ProblemCode::InvalidInput,
			"--undo, --yes and --force currently apply only to removal recovery.",
		)));
	}

	let result = workyard_core::recover(
		&name,
		&accepted,
		&runner,
		&control,
		&Events(context, "recover"),
		&store,
	)
	.await;
	match result {
		Ok(result) => {
			let mut outcome = RecoverOutcome::from_task(result.task, result.record);
			outcome.nothing_to_recover = result.nothing_to_recover;
			if outcome.nothing_to_recover {
				outcome.operation.continues = outcome
					.operation
					.continues
					.or(outcome.operation.kind.filter(|kind| *kind != "recover"));
				outcome.operation.kind = Some("recover");
			}
			if outcome.operation.outcome == OperationOutcome::Failed {
				Err(outcome)
			} else {
				Ok(outcome)
			}
		}
		Err(problem) if problem.code == ProblemCode::Cancelled => Ok(rejected(problem)),
		Err(problem) => Err(rejected(problem)),
	}
}
