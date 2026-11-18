use schemars::JsonSchema;
use serde::Deserialize;
use trine::NativePath;

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RelocateInput {
	/// Source workspace root.
	pub old: Option<NativePath>,
	pub old_tasks_root: Option<NativePath>,
	/// Destination workspace root.
	pub new: Option<NativePath>,
	pub new_tasks_root: Option<NativePath>,
	/// Locate the source by its state directory.
	pub old_state_dir: Option<NativePath>,
	/// Destination for workspace state.
	pub new_state_dir: Option<NativePath>,
	/// Move workspace files or their state directory (default).
	#[serde(default, rename = "move")]
	pub move_workspace: bool,
	/// Keep the source files as an inactive snapshot after moving the active workspace.
	#[serde(default, rename = "keep_source", alias = "copy")]
	pub copy: bool,
}
