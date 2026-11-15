//! A final task publication may carry identity learned after the success receipt.
use crate::{RecoveryRecord, RemovalPhase, Task};

pub(in crate::remove) fn retain_recorded_identity(record: &RecoveryRecord, task: &mut Task) {
	let Some(recorded) = &record.recorded else {
		return;
	};
	let Some(current_removal) = &task.removal else {
		return;
	};
	let Some(recorded_removal) = &recorded.removal else {
		return;
	};
	for (index, participant) in task.participants.iter_mut().enumerate() {
		let current = &current_removal.participants[index];
		let published = &recorded_removal.participants[index];
		let saved = &recorded.participants[index];
		if current.phase != RemovalPhase::Recreated || participant.worktree_identity.is_some() {
			continue;
		}
		if !matches!(
			published.phase,
			RemovalPhase::Recreated | RemovalPhase::Restored
		) || current.branch != published.branch
			|| participant.destination != saved.destination
		{
			continue;
		}
		participant.worktree_identity = saved.worktree_identity.clone();
	}
}
