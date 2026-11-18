//! Registered interface routing preserves the public task-specific Rust API.
use super::{
	RecoverInput,
	interface::{Input, Outcome},
};
use crate::{
	actions::relocate::{RelocateOutcome, recover_workspace},
	contracts::Progress,
};
use trine::{InvocationContext, NativePath};
use workyard_core::{Problem, ProblemCode};

pub(super) async fn recover(
	input: Input,
	context: InvocationContext<Progress>,
) -> Result<Outcome, Outcome> {
	if let Some(task) = input.task {
		return super::recover(
			RecoverInput {
				task,
				tasks_root: input.tasks_root,
				state_dir: input.state_dir,
				accept_pull: input.accept_pull,
				undo: input.undo,
				yes: input.yes,
				force: input.force,
			},
			context,
		)
		.await
		.map(Outcome::Task)
		.map_err(Outcome::Task);
	}
	let rejected = |problem| Outcome::Workspace(RelocateOutcome::rejected("recover", problem));
	if !input.accept_pull.is_empty() || input.force || input.yes {
		return Err(rejected(Problem::new(
			ProblemCode::InvalidInput,
			"Workspace recovery does not accept task consent or pull flags.",
		)));
	}
	let location = crate::workspace::placement::existing(
		input.tasks_root.as_ref().map(NativePath::as_path),
		input.state_dir.as_ref().map(NativePath::as_path),
	)
	.map_err(rejected)?;
	recover_workspace(&location.state, input.undo, context)
		.await
		.map(Outcome::Workspace)
		.map_err(Outcome::Workspace)
}
