//! Live participant messages for subscribed MCP tool calls.
use crate::{contracts::Progress, presentation::structured::EVENT_CAPACITY};
use trine::structured::StructuredBinding;

pub(crate) fn progress_binding<O, E>() -> StructuredBinding<O, E, Progress>
where
	O: serde::Serialize + Sync + 'static,
	E: serde::Serialize + Sync + 'static,
{
	StructuredBinding::serde_with_progress(|event| Ok(message(event).into()), EVENT_CAPACITY)
}

fn message(event: Progress) -> String {
	let participant = event.participant;
	match event.phase.as_str() {
		"fetching" => format!("Fetching {participant}"),
		"creating" => format!("Creating worktree for {participant}"),
		"worktree_created" => format!("Created worktree for {participant}"),
		"completed" => format!("Finished worktree setup for {participant}"),

		"added" => format!("Added participant {participant}"),
		"renamed" => format!("Renamed participant to {participant}"),
		"updated" => format!("Updated participant {participant}"),
		"unchanged" => format!("Checked participant {participant} (unchanged)"),
		"inspected" => format!("Inspected worktree for {participant}"),
		"removed" if event.action == "remove" => format!("Unlinked worktree for {participant}"),
		"removed" => format!("Unlinked original worktree for {participant}"),

		"repairing_worktree_link" => format!("Repairing Git worktree link for {participant}"),
		"repaired" => format!("Repaired Git worktree link for {participant}"),
		"updating_worktree_location" => {
			format!("Updating saved worktree location for {participant}")
		}
		_ => format!("Processing worktree for {participant}"),
	}
}
