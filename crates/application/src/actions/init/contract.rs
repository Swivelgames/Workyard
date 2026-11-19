use serde::Serialize;
use trine::NativePath;

#[derive(Serialize)]
pub struct InitOutcome {
	pub workspace: NativePath,
	pub tasks_root: NativePath,
}
