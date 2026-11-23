use super::{
	super::contract::{Inventory, Project, Root},
	project_command,
};
use crate::{
	contracts::{ProblemOutcome, Progress},
	guidance::text::path,
	presentation::cli::render::{MessageScope, Terminal, Tone, separate},
};
use std::{fmt::Write, io, path::Path};
use trine::{application::CliViewContext, view::View};

pub(in crate::actions::projects) struct ListView {
	pub output: CliViewContext,
}

impl View<Inventory, ProblemOutcome, Progress> for ListView {
	type Completion = ();
	type Error = io::Error;

	async fn finish(&mut self, result: &Result<Inventory, ProblemOutcome>) -> io::Result<()> {
		let mut terminal = Terminal::for_output(&self.output.stdout);
		match result {
			Err(problem) => {
				terminal.heading("Could not list projects");
				separate(&mut terminal);
				terminal.problem_at(MessageScope::Section, problem);
			}
			Ok(inventory) => render_inventory(&mut terminal, inventory),
		}
		self.output.stdout.write_all(terminal.output.as_bytes())
	}
}

fn render_inventory(terminal: &mut Terminal, inventory: &Inventory) {
	if let Some(hint) = &inventory.guidance {
		terminal.hint_at(MessageScope::Workspace, hint);
		return;
	}
	let workspace = inventory.workspace.as_ref().map(|path| path.as_path());
	let default_root = workspace.map(|workspace| workspace.join("projects"));
	let roots: Vec<_> = inventory
		.roots
		.iter()
		.filter(|root| {
			root.status != "missing" || default_root.as_deref() != Some(root.path.as_path())
		})
		.collect();
	if roots.is_empty() && inventory.projects.is_empty() {
		terminal.heading("No projects found");
		separate(terminal);
		let advice = if roots.len() == inventory.roots.len() {
			"Track a project directory"
		} else {
			"Add repositories under projects/ or track a project directory"
		};
		terminal.note_at(MessageScope::Workspace, advice);
		separate(terminal);
		let workspace = inventory
			.explicit_workspace
			.as_ref()
			.map(|path| path.as_path());
		project_command(terminal, "projects track", workspace, " <path>");
		if workspace.is_some() {
			terminal.note_at(
				MessageScope::Section,
				"Replace <path> with your project directory.",
			);
		}
		return;
	}

	let mut groups = vec![Vec::new(); roots.len()];
	let mut ungrouped = Vec::new();
	for project in &inventory.projects {
		match project_root(project, &roots) {
			Some(index) => groups[index].push(project),
			None => ungrouped.push(project),
		}
	}

	terminal.heading("Project list");
	for (root, projects) in roots.into_iter().zip(groups) {
		separate(terminal);
		render_root(terminal, workspace, root, &projects);
	}
	if !ungrouped.is_empty() {
		separate(terminal);
		terminal.note_at(MessageScope::Section, "Other discovered projects");
		render_projects(terminal, &ungrouped);
	}
}

fn project_root(project: &Project, roots: &[&Root]) -> Option<usize> {
	roots
		.iter()
		.enumerate()
		.filter(|(_, root)| project.path.as_path().starts_with(root.path.as_path()))
		.max_by_key(|(_, root)| root.path.as_path().components().count())
		.map(|(index, _)| index)
}

fn render_root(
	terminal: &mut Terminal,
	workspace: Option<&Path>,
	root: &Root,
	projects: &[&Project],
) {
	let root_path = root.path.as_path();
	let label = match workspace.filter(|workspace| root_path.starts_with(workspace)) {
		Some(workspace) => terminal.paths.within(workspace, root_path),
		None => terminal.paths.root(root_path),
	};
	let separator = if label.ends_with(['/', std::path::MAIN_SEPARATOR]) {
		""
	} else {
		"/"
	};
	let label = terminal
		.style
		.paint(&format!("{label}{separator}"), Tone::Identity);
	write!(terminal.output, "    {label}").expect("string output");

	if root.status != "available" {
		let status = terminal.tag(root.status, Tone::Attention);
		write!(terminal.output, " {status}").expect("string output");
	}
	terminal.output.push('\n');

	if projects.is_empty() && root.status == "available" {
		terminal.note_at(MessageScope::Task, "No projects");
	}
	render_projects(terminal, projects);
	for issue in &root.issues {
		terminal.problem(issue);
	}
}

fn render_projects(terminal: &mut Terminal, projects: &[&Project]) {
	for project in projects {
		let reference = path(project.reference.as_path());
		writeln!(terminal.output, "      {reference}").expect("string output");
	}
}
