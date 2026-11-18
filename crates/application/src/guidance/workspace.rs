//! Next steps for workspace discovery and relocation.
use crate::guidance::commands;
use std::path::Path;

pub(crate) fn missing_state(root: Option<&Path>, explicit: Option<&Path>) -> String {
	getting_started("No Workyard state found.", root, explicit)
}

pub(crate) fn empty_workspace(state: &Path, root: &Path) -> String {
	getting_started("No tasks found.", Some(root), Some(state))
}

pub(crate) fn snapshot(snapshot: &crate::contracts::relocation::WorkspaceSnapshot) -> String {
	let active = snapshot.active_tasks_root.as_path();
	let root = commands::path(active).unwrap_or_else(|| format!("{active:?}"));
	let inspect = commands::workspace_status(Some(snapshot.active_state_dir.as_path()));
	let location = format!("{}. Active workspace: {root}", snapshot.label());
	match inspect {
		Some(command) => format!("{location}\nInspect: {command}"),
		None => location,
	}
}

pub(crate) fn relocation(state: &Path, placement_complete: bool, copy: bool) -> String {
	let command = commands::path(state)
		.map(|path| format!("workyard recover --state-dir={path}"))
		.unwrap_or_else(|| "workyard recover with the same state directory".into());
	if placement_complete && copy {
		return format!("Placement complete. Finish marking the snapshot with {command}");
	}
	if placement_complete {
		return format!("Placement complete. Finish cleanup with {command}");
	}
	format!(
		"Relocation in progress. Continue with {command} or add --undo to return to the original root."
	)
}

fn getting_started(message: &str, root: Option<&Path>, state: Option<&Path>) -> String {
	let Some(command) = create_command(root, state) else {
		return format!(
			"{message}\nGet started with workyard create --repo /path/to/repo feat/example; supply --tasks-root and --state-dir using your shell's native-path quoting."
		);
	};
	format!("{message}\nGet started: {command}")
}

fn create_command(root: Option<&Path>, state: Option<&Path>) -> Option<String> {
	let mut command = String::from("workyard create --repo /path/to/repo");
	if let Some(root) = root {
		command.push_str(&format!(" --tasks-root={}", commands::path(root)?));
	}
	if let Some(state) = state {
		command.push_str(&format!(" --state-dir={}", commands::path(state)?));
	}
	Some(format!("{command} feat/example"))
}
