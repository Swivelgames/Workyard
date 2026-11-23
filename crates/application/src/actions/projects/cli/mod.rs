mod inventory;
mod roots;

pub(super) use inventory::ListView;
pub(super) use roots::RootsView;

use crate::presentation::cli::render::{MessageScope, Terminal};
use std::path::Path;

fn project_command(terminal: &mut Terminal, action: &str, workspace: Option<&Path>, suffix: &str) {
	if let Some(command) = crate::guidance::commands::workspace_command(action, workspace) {
		terminal.command_at(MessageScope::Section, &format!("{command}{suffix}"));
	} else {
		terminal.note_at(
			MessageScope::Section,
			&format!("Run workyard {action} from the selected workspace."),
		);
	}
}
