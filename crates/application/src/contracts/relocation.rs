use serde::Serialize;
use trine::NativePath;

#[derive(Serialize)]
pub struct WorkspaceRelocation {
	pub operation_id: String,
	pub old_tasks_root: NativePath,
	pub new_tasks_root: NativePath,
	pub old_state_dir: NativePath,
	pub new_state_dir: NativePath,
	pub placement_complete: bool,
	pub writer_pending: bool,
	pub authority: String,
}

impl From<workyard_state::RelocationStatus> for WorkspaceRelocation {
	fn from(status: workyard_state::RelocationStatus) -> Self {
		Self {
			operation_id: status.operation_id,
			old_tasks_root: NativePath::new(status.old_root).expect("validated native anchor"),
			new_tasks_root: NativePath::new(status.new_root).expect("validated native anchor"),
			old_state_dir: NativePath::new(status.old_state).expect("validated native anchor"),
			new_state_dir: NativePath::new(status.new_state).expect("validated native anchor"),
			placement_complete: status.placement_complete,
			writer_pending: status.writer_pending,
			authority: status.authority,
		}
	}
}
