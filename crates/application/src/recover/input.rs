use schemars::JsonSchema;
use serde::Deserialize;
use trine::NativePath;

/// Resolved placement for an existing task; this never initializes a workspace.
pub(super) struct RecoveryLocation {
	pub root: Option<std::path::PathBuf>,
	pub state: std::path::PathBuf,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RecoverInput {
	pub task: String,
	pub tasks_root: Option<NativePath>,
	pub state_dir: Option<NativePath>,
	#[serde(default)]
	pub accept_pull: Vec<String>,
}

impl RecoverInput {
	pub(super) fn location(&self) -> Result<RecoveryLocation, workyard_core::Problem> {
		let cwd = std::env::current_dir()
			.map_err(|error| workyard_core::Problem::io("Read current directory", error))?;
		let root = self
			.tasks_root
			.as_ref()
			.map(|path| path.as_path().to_path_buf());
		let state = workyard_state::discover_state(
			root.as_deref().unwrap_or(&cwd),
			self.state_dir.as_ref().map(|path| path.as_path()),
		)?;
		Ok(RecoveryLocation { root, state })
	}
}
