//! Record supervised add completion before attempting secondary identity reads.
use crate::{Git, Phase, Problem, State, Task};

pub(crate) async fn settle_add<S: State>(
	result: Result<(), Problem>,
	task: &mut Task,
	index: usize,
	runner: &impl Git,
	store: &S,
	guard: &S::Admission,
) -> Result<(), Problem> {
	if result.is_ok() {
		task.participants[index].phase = Phase::WorktreeCreated;
	}
	// Process supervision has finished. Identity inspection failure must not
	// turn this observed settlement back into an uncertain writer diagnosis.
	store.record_effect(guard, task, index, crate::EffectState::Settled)?;
	result?;

	let identity = runner
		.worktree_identity(&task.participants[index].destination)
		.await
		.map_err(|mut problem| {
			problem.remedy.push_str(
				" Git worktree creation completed and managed writers stopped, but its administrative identity could not be recorded. Preserve the worktree and inspect Git relationships before recovery.",
			);
			problem
		})?;
	task.participants[index].worktree_identity = identity;
	store.record_effect(guard, task, index, crate::EffectState::Settled)
}
