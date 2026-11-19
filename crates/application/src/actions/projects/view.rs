use super::contract::{Inventory, RootChanges};
use crate::{
	contracts::{ProblemOutcome, Progress},
	guidance::text::path,
	presentation::cli::render::{MessageScope, Terminal},
};
use std::{fmt::Write, io};
use trine::{application::CliViewContext, view::View};
use workyard_state::RootEdit;

pub(super) struct ListView(pub CliViewContext);
pub(super) struct RootsView {
	pub output: CliViewContext,
	pub edit: RootEdit,
}

impl View<Inventory, ProblemOutcome, Progress> for ListView {
	type Completion = ();
	type Error = io::Error;
	async fn progress(&mut self, _: Progress) -> io::Result<()> {
		Ok(())
	}
	async fn finish(&mut self, result: &Result<Inventory, ProblemOutcome>) -> io::Result<()> {
		let mut terminal = Terminal::for_output(&self.0.stdout);
		match result {
			Err(problem) => terminal.problem_at(MessageScope::Workspace, problem),
			Ok(inventory) => render_inventory(&mut terminal, inventory),
		}
		self.0.stdout.write_all(terminal.output.as_bytes())
	}
}

fn render_inventory(terminal: &mut Terminal, inventory: &Inventory) {
	if let Some(hint) = &inventory.guidance {
		terminal.hint_at(MessageScope::Workspace, hint);
		return;
	}
	if inventory.projects.is_empty() {
		terminal.heading("No projects found.");
	} else {
		terminal.heading("Projects");
		for project in &inventory.projects {
			writeln!(
				terminal.output,
				"    {}    {}",
				path(project.reference.as_path()),
				terminal.paths.root(project.path.as_path())
			)
			.expect("string output");
		}
	}
	if inventory.roots.is_empty() {
		terminal.hint_at(
			MessageScope::Workspace,
			"Register a directory with workyard projects track <path>.",
		);
		return;
	}
	terminal.output.push('\n');
	terminal.heading("Tracked roots");
	for root in &inventory.roots {
		write!(
			terminal.output,
			"    {}",
			terminal.paths.root(root.path.as_path())
		)
		.expect("string output");
		if root.status != "available" {
			write!(terminal.output, " [{}]", root.status).expect("string output");
		}
		terminal.output.push('\n');
		for issue in &root.issues {
			terminal.problem(issue);
		}
	}
}

impl View<RootChanges, ProblemOutcome, Progress> for RootsView {
	type Completion = ();
	type Error = io::Error;
	async fn progress(&mut self, _: Progress) -> io::Result<()> {
		Ok(())
	}
	async fn finish(&mut self, result: &Result<RootChanges, ProblemOutcome>) -> io::Result<()> {
		let mut terminal = Terminal::for_output(&self.output.stdout);
		match result {
			Err(problem) => terminal.problem_at(MessageScope::Workspace, problem),
			Ok(outcome) if outcome.changed.is_empty() => {
				terminal.heading("No changes to tracked roots.")
			}
			Ok(outcome) => {
				let verb = match self.edit {
					RootEdit::Track => "Tracked",
					RootEdit::Untrack => "Untracked",
				};
				for root in &outcome.changed {
					writeln!(
						terminal.output,
						"{verb} {}",
						terminal.paths.root(root.as_path())
					)
					.expect("string output");
				}
			}
		}
		self.output.stdout.write_all(terminal.output.as_bytes())
	}
}
