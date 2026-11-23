//! Shell-ready next steps for absent and empty workspaces.
use crate::guidance::commands;
use std::path::Path;

pub(crate) fn missing_state(workspace: Option<&Path>) -> String {
	format!("No Workyard workspace found. {}", init_guidance(workspace))
}

pub(crate) fn init_guidance(workspace: Option<&Path>) -> String {
	let target = workspace.and_then(commands::path);
	let command = match (workspace, target) {
		(None, _) => "workyard init".to_owned(),
		(_, Some(target)) => format!("workyard init -- {target}"),
		_ => "workyard init from the intended workspace directory".to_owned(),
	};
	format!("Get started: {command}")
}

pub(crate) fn empty_workspace(workspace: Option<&Path>) -> String {
	let Some(command) = commands::workspace_command("create", workspace) else {
		return "No tasks found. Run workyard create from the selected workspace directory.".into();
	};
	format!("No tasks found. Get started: {command} --repo /path/to/repo feat/example")
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn an_unquotable_explicit_selection_keeps_guidance_in_that_workspace() {
		let guidance = empty_workspace(Some(Path::new("workspace\nname")));
		assert!(guidance.contains("from the selected workspace directory"));
		assert!(!guidance.contains("--repo"));
	}
}
