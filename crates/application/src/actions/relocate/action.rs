use super::{RelocateInput, RelocateOutcome};
use crate::{
	contracts::{Progress, relocation::WorkspaceSnapshot},
	execution::blocking_action,
};
use std::path::{Path, PathBuf};
use trine::{InvocationContext, NativePath, invocation::Confirmation};
use workyard_core::{Control, Problem, ProblemCode, RelocationState};
use workyard_state::{Relocation, canonical_future};

struct Prepared {
	state: PathBuf,
	old_root: PathBuf,
	target: PathBuf,
	confirm_source: bool,
}

pub async fn relocate(
	input: RelocateInput,
	context: InvocationContext<Progress>,
) -> Result<RelocateOutcome, RelocateOutcome> {
	blocking_action(async move {
		let copy = input.copy;
		run(input, context).await.map_err(|problem| {
			let mut outcome = RelocateOutcome::rejected("relocate", problem);
			outcome.copy = copy;
			outcome
		})?
	})
	.await
}

async fn run(
	input: RelocateInput,
	context: InvocationContext<Progress>,
) -> Result<Result<RelocateOutcome, RelocateOutcome>, Problem> {
	let cancellation = context.cancellation().clone();
	let control = Control::new(move || cancellation.is_cancelled());
	control.check()?;

	let prepared = prepare(&input)?;
	if prepared.confirm_source {
		confirm_source(&prepared.old_root, input.copy, &context).await?;
	}

	control.check()?;
	let open = if input.copy {
		Relocation::open_copy_control
	} else {
		Relocation::open_control
	};
	let mut session = open(
		&prepared.state,
		Some(&prepared.old_root),
		&prepared.target,
		input.new_state_dir.as_ref().map(NativePath::as_path),
		&control,
	)?;
	if session.old_root()? == session.new_root()? && session.old_state()? == session.new_state()? {
		let mut outcome = outcome(&session, "relocate")?;
		outcome.nothing_to_recover = true;
		outcome.placement_complete = true;
		outcome.cleanup_complete = true;
		return Ok(Ok(outcome));
	}
	Ok(execute(&mut session, &control, false, "relocate", context).await)
}

fn prepare(input: &RelocateInput) -> Result<Prepared, Problem> {
	if input.copy && input.move_workspace {
		return Err(Problem::new(
			ProblemCode::InvalidInput,
			"Choose either a regular move or a relocation that keeps the source files.",
		));
	}

	let cwd =
		std::env::current_dir().map_err(|error| Problem::io("Read current directory", error))?;
	let old = alias(input.old.as_ref(), input.old_tasks_root.as_ref())?;
	let new = alias(input.new.as_ref(), input.new_tasks_root.as_ref())?;
	let state = workyard_state::discover_state(
		old.as_deref().unwrap_or(&cwd),
		input.old_state_dir.as_ref().map(NativePath::as_path),
	)?;
	let reader = workyard_state::Reader::open(&state, old.as_deref())?;
	let old_root = reader.root().to_path_buf();
	drop(reader);

	let target = if let Some(new) = new {
		new
	} else if input.new_state_dir.is_some() {
		old_root.clone()
	} else {
		match workyard_state::find_state(&cwd, None)? {
			Some(state) => workyard_state::Reader::open(&state, None)?
				.root()
				.to_path_buf(),
			None => cwd,
		}
	};
	Ok(Prepared {
		state,
		old_root,
		target,
		confirm_source: old.is_none(),
	})
}

async fn confirm_source(
	old_root: &Path,
	copy: bool,
	context: &InvocationContext<Progress>,
) -> Result<(), Problem> {
	let source = crate::guidance::text::path(old_root);
	let question = if copy {
		format!("Move the workspace at {source} and keep its source files?")
	} else {
		format!("Move the workspace at {source}?")
	};
	let answer = context
		.confirm(Confirmation {
			code: "relocate_source".into(),
			message: question,
		})
		.await
		.map_err(|_| Problem::new(ProblemCode::Cancelled, "Source confirmation interrupted."))?;
	match answer {
		Some(true) => {}
		Some(false) => {
			return Err(Problem::new(
				ProblemCode::Cancelled,
				"Relocation declined; workspace unchanged.",
			));
		}
		None => {
			return Err(Problem::new(
				ProblemCode::DecisionRequired,
				"Confirm the discovered source or explicitly select --old/old_tasks_root before relocation.",
			));
		}
	}

	Ok(())
}

pub(crate) async fn recover_workspace(
	state: &Path,
	undo: bool,
	context: InvocationContext<Progress>,
) -> Result<RelocateOutcome, RelocateOutcome> {
	let state = state.to_path_buf();
	blocking_action(async move {
		let cancellation = context.cancellation().clone();
		let control = Control::new(move || cancellation.is_cancelled());
		control
			.check()
			.map_err(|problem| RelocateOutcome::rejected("recover", problem))?;
		if workyard_state::relocation_status(&state)
			.map_err(|problem| RelocateOutcome::rejected("recover", problem))?
			.is_none()
		{
			let reader = workyard_state::Reader::open(&state, None)
				.map_err(|problem| RelocateOutcome::rejected("recover", problem))?;
			if undo {
				return Err(RelocateOutcome::rejected(
					"recover",
					Problem::new(
						ProblemCode::InvalidInput,
						"No pending relocation can be undone.",
					),
				));
			}
			let mut outcome = RelocateOutcome::empty("recover");
			outcome.nothing_to_recover = true;
			outcome.placement_complete = true;
			outcome.cleanup_complete = true;
			outcome.new_tasks_root = Some(NativePath::new(reader.root()).expect("root path"));
			outcome.new_state_dir = Some(NativePath::new(reader.state()).expect("state path"));
			return Ok(outcome);
		}
		let mut session = Relocation::recover(&state)
			.map_err(|problem| RelocateOutcome::rejected("recover", problem))?;
		execute(&mut session, &control, undo, "recover", context).await
	})
	.await
}

async fn execute(
	session: &mut Relocation,
	control: &Control,
	undo: bool,
	kind: &'static str,
	context: InvocationContext<Progress>,
) -> Result<RelocateOutcome, RelocateOutcome> {
	let observer = super::interaction::RelocationEvents::new(context, kind);
	let result = workyard_core::relocate(
		session,
		&workyard_git::Runner::default(),
		control,
		undo,
		&observer,
	)
	.await;
	let mut outcome =
		outcome(session, kind).unwrap_or_else(|problem| RelocateOutcome::rejected(kind, problem));
	outcome.recovery_choice = observer.pending.into_inner();
	match result {
		Ok(()) => Ok(outcome),
		Err(problem) => {
			outcome.fail(problem);
			Err(outcome)
		}
	}
}

fn outcome(session: &Relocation, kind: &'static str) -> Result<RelocateOutcome, Problem> {
	let mut outcome = RelocateOutcome::empty(kind);
	let old_root = session.old_root()?;
	outcome.copy = session.is_copy();
	outcome.operation_id = Some(session.operation_id().into());
	outcome.mutation_started = session.started();
	outcome.old_tasks_root = Some(native(&old_root)?);
	outcome.new_tasks_root = Some(native(&session.new_root()?)?);
	outcome.old_state_dir = Some(native(&session.old_state()?)?);
	outcome.new_state_dir = Some(native(&session.new_state()?)?);
	outcome.placement_complete = session.placement_complete();
	if outcome.placement_complete {
		outcome.active_tasks_root = Some(native(&session.active_root()?)?);
		outcome.active_state_dir = Some(native(&session.active_state()?)?);
	}
	outcome.cleanup_complete = session.cleanup_complete();
	if outcome.copy && outcome.cleanup_complete {
		outcome.snapshot = Some(retained_snapshot(session)?);
	}
	for entry in session.participants() {
		if !entry.original.destination.starts_with(&old_root) {
			outcome.warnings.push(super::contract::ExternalParticipant {
				task: entry.task,
				participant: entry.original.name,
				location: native(&entry.original.destination)?,
			});
		}
	}
	Ok(outcome)
}

fn retained_snapshot(session: &Relocation) -> Result<WorkspaceSnapshot, Problem> {
	let active_root = session.active_root()?;
	let partial = active_root == session.old_root()?;
	let (retained_root, retained_state) = if partial {
		(session.new_root()?, session.new_state()?)
	} else {
		(session.old_root()?, session.old_state()?)
	};
	Ok(WorkspaceSnapshot {
		operation_id: session.operation_id().into(),
		tasks_root: native(&retained_root)?,
		state_dir: native(&retained_state)?,
		active_tasks_root: native(&active_root)?,
		active_state_dir: native(&session.active_state()?)?,
		partial,
	})
}

fn native(path: &Path) -> Result<NativePath, Problem> {
	NativePath::new(path)
		.map_err(|error| Problem::new(ProblemCode::InvalidInput, error.to_string()))
}

fn alias(
	first: Option<&NativePath>,
	second: Option<&NativePath>,
) -> Result<Option<PathBuf>, Problem> {
	let first = first
		.map(|path| canonical_future(path.as_path()))
		.transpose()?;
	let second = second
		.map(|path| canonical_future(path.as_path()))
		.transpose()?;
	if first.is_some() && second.is_some() && first != second {
		return Err(Problem::new(
			ProblemCode::InvalidInput,
			"Conflicting root alias values.",
		));
	}
	Ok(first.or(second))
}
