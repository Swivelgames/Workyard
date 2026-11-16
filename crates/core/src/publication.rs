//! Shared publication mechanics; callers own cancellation and lifecycle policy.
use crate::{EffectState, Observer, Phase, Problem, State, Task};

pub(crate) async fn phase<S: State>(
	task: &mut Task,
	index: usize,
	phase: Phase,
	store: &S,
	guard: &S::Admission,
	observer: &impl Observer,
) -> Result<(), Problem> {
	task.participants[index].phase = phase;
	store.record_effect(guard, task, index, EffectState::Settled)?;
	store.save(guard, task)?;
	observer
		.phase(
			&task.name,
			&task.participants[index].name,
			task.participants[index].phase.as_str(),
		)
		.await
}
