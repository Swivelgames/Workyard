use super::{super::contract::RootChanges, project_command};
use crate::{
	contracts::{ProblemOutcome, Progress},
	presentation::cli::render::{MessageScope, Terminal, separate},
};
use std::io;
use trine::{application::CliViewContext, view::View};
use workyard_state::RootEdit;

pub(in crate::actions::projects) struct RootsView {
	pub output: CliViewContext,
	pub edit: RootEdit,
}

impl View<RootChanges, ProblemOutcome, Progress> for RootsView {
	type Completion = ();
	type Error = io::Error;

	async fn finish(&mut self, result: &Result<RootChanges, ProblemOutcome>) -> io::Result<()> {
		let mut terminal = Terminal::for_output(&self.output.stdout);
		match result {
			Err(problem) => {
				let title = match self.edit {
					RootEdit::Track => "Could not track project roots",
					RootEdit::Untrack => "Could not untrack project roots",
				};
				terminal.heading(title);
				separate(&mut terminal);
				if problem.code == "publication_uncertain" {
					terminal.note_at(
						MessageScope::Section,
						"Could not confirm that the project roots were saved.",
					);
					terminal.note_at(
						MessageScope::Section,
						"Check access to .workyard and inspect the tracked roots.",
					);
					let workspace = problem
						.explicit_workspace
						.as_ref()
						.map(|path| path.as_path());
					project_command(&mut terminal, "projects list", workspace, "");
				} else {
					terminal.problem_at(MessageScope::Section, problem);
				}
			}
			Ok(outcome) if outcome.changed.is_empty() => {
				terminal.heading("No changes to tracked roots.")
			}
			Ok(outcome) => {
				let title = match self.edit {
					RootEdit::Track => "Tracked project roots",
					RootEdit::Untrack => "Stopped tracking project roots",
				};
				terminal.heading(title);
				separate(&mut terminal);

				for root in &outcome.changed {
					let label = terminal.paths.root(root.as_path());
					terminal.note_at(MessageScope::Section, &label);
				}
			}
		}
		self.output.stdout.write_all(terminal.output.as_bytes())
	}
}
