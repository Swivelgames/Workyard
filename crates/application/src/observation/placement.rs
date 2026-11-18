//! Read the selected endpoint before opening it as an ordinary workspace.
use super::Observation;
use crate::{contracts::relocation::WorkspaceSnapshot, guidance};
use std::path::Path;
use trine::NativePath;
use workyard_core::Problem;

pub(crate) fn inspect(state: &Path) -> Result<Option<Observation>, Problem> {
	if let Some(snapshot) = workyard_state::snapshot_status(state)? {
		let snapshot = WorkspaceSnapshot {
			operation_id: snapshot.operation_id,
			tasks_root: native(&snapshot.tasks_root),
			state_dir: native(state),
			active_tasks_root: native(&snapshot.active_tasks_root),
			active_state_dir: native(&snapshot.active_state_dir),
			partial: snapshot.partial,
		};
		let guidance = guidance::workspace::snapshot(&snapshot);
		return Ok(Some(Observation {
			tasks_root: Some(snapshot.tasks_root.clone()),
			state_dir: Some(snapshot.state_dir.clone()),
			guidance: Some(guidance),
			snapshot: Some(snapshot),
			..Default::default()
		}));
	}
	let Some(status) = workyard_state::relocation_status(state)? else {
		return Ok(None);
	};
	Ok(Some(Observation {
		state_dir: Some(native(state)),
		tasks_root: Some(native(&status.tasks_root)),
		guidance: Some(guidance::workspace::relocation(
			state,
			status.placement_complete,
			status.copy,
		)),
		relocation: Some(status.into()),
		..Default::default()
	}))
}

fn native(path: &Path) -> NativePath {
	NativePath::new(path).expect("validated workspace path")
}
