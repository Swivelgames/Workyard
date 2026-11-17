use schemars::JsonSchema;
use serde::Deserialize;
use trine::NativePath;

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RemoveInput {
	pub task: String,
	pub tasks_root: Option<NativePath>,
	pub state_dir: Option<NativePath>,
	#[serde(default)]
	pub yes: bool,
	#[serde(default)]
	pub force: bool,
}

impl RemoveInput {
	pub(super) fn location(
		&self,
	) -> Result<crate::workspace::placement::Existing, workyard_core::Problem> {
		crate::workspace::placement::existing(
			self.tasks_root.as_ref().map(NativePath::as_path),
			self.state_dir.as_ref().map(NativePath::as_path),
		)
	}
}
