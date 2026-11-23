use super::super::InitOutcome;
use crate::{
	contracts::{ProblemOutcome, Progress},
	guidance::commands,
	presentation::cli::render::{MessageScope, Terminal, Tone, separate},
};
use std::{fmt::Write, io, path::Path};
use trine::{application::CliViewContext, view::View};
use workyard_core::{Control, projects::Project};

pub struct InitView {
	pub output: CliViewContext,
}

impl View<InitOutcome, ProblemOutcome, Progress> for InitView {
	type Completion = ();
	type Error = io::Error;

	async fn finish(&mut self, result: &Result<InitOutcome, ProblemOutcome>) -> io::Result<()> {
		let mut terminal = Terminal::for_output(&self.output.stdout);

		match result {
			Ok(outcome) => {
				let workspace = outcome.workspace.as_path();
				layout(&mut terminal, workspace, outcome.tasks_root.as_path());
				if !outcome.tasks_root.as_path().starts_with(workspace) {
					separate(&mut terminal);
					terminal.advice_at(
						MessageScope::Workspace,
						"Use --workspace on every command in this detached workspace.",
						None,
					);
					if let Some(command) = commands::workspace_command("list", Some(workspace)) {
						separate(&mut terminal);
						terminal.command_at(MessageScope::Section, &command);
					}
					return self.output.stdout.write_all(terminal.output.as_bytes());
				}

				self.output.stdout.write_all(terminal.output.as_bytes())?;
				terminal.output.clear();

				let control = Control::new({
					let cancellation = outcome.cancellation.clone();
					move || cancellation.is_cancelled()
				});
				let discovery = discover_projects(workspace, &control).await;
				if outcome.cancellation.is_cancelled() {
					return Ok(());
				}

				let mut projects: Vec<_> = discovery
					.iter()
					.flat_map(|discovery| discovery.catalog.projects())
					.collect();
				projects.sort_by(|left, right| left.reference.cmp(&right.reference));
				project_preview(&mut terminal, workspace, &projects);
				next_steps(&mut terminal, workspace, &projects);
			}
			Err(problem) => {
				terminal.heading("Could not initialize workspace");
				separate(&mut terminal);
				terminal.problem_at(MessageScope::Section, problem);
			}
		}

		self.output.stdout.write_all(terminal.output.as_bytes())
	}
}

fn layout(terminal: &mut Terminal, workspace: &Path, tasks: &Path) {
	let location = terminal.paths.root(workspace);
	let title = if location == "." {
		"Workspace initialized".into()
	} else {
		format!("Workspace initialized at {location}")
	};
	let title = terminal.style.paint(&title, Tone::SuccessTitle);
	let state = terminal.style.paint(".workyard/", Tone::Section);
	let tasks = terminal.paths.within(workspace, tasks);
	let tasks = terminal.style.paint(&format!("{tasks}/"), Tone::Section);

	writeln!(terminal.output, "  {title}\n\n    {state}\n    {tasks}").expect("string output");
}

async fn discover_projects(
	workspace: &Path,
	control: &Control,
) -> Option<workyard_git::projects::Discovery> {
	control.check().ok()?;
	let reader = workyard_state::Reader::open(&workspace.join(".workyard"), None).ok()?;
	crate::projects::discover(&reader, control).await.ok()
}

fn project_preview(terminal: &mut Terminal, workspace: &Path, projects: &[&Project]) {
	const PREVIEW_LIMIT: usize = 4;
	let count = projects.len();
	if count == 0 {
		return;
	}

	let label = if count == 1 { "project" } else { "projects" };
	let title = terminal
		.style
		.paint(&format!("Found {count} {label}"), Tone::Hint);
	writeln!(terminal.output, "\n  {title}\n").expect("string output");

	for project in projects.iter().take(PREVIEW_LIMIT) {
		let path = terminal.paths.within(workspace, &project.path);
		let path = terminal.style.paint(&format!("{path}/"), Tone::Identity);
		writeln!(terminal.output, "    {path}").expect("string output");
	}
	if count > PREVIEW_LIMIT {
		let remaining = count - PREVIEW_LIMIT;
		writeln!(terminal.output, "    … and {remaining} more").expect("string output");
	}
}

fn next_steps(terminal: &mut Terminal, workspace: &Path, projects: &[&Project]) {
	if terminal.paths.root(workspace) != "." {
		writeln!(
			terminal.output,
			"\n  Run these commands from the workspace directory."
		)
		.expect("string output");
	}

	let selectors: Vec<_> = projects
		.iter()
		.filter_map(|project| project.reference.to_str())
		.filter(|reference| !reference.starts_with('-'))
		.filter_map(commands::argument)
		.take(2)
		.collect();

	let (instruction, selectors) = if selectors.is_empty() {
		(
			"Once tracked, create a task branch using your project names",
			"api web".into(),
		)
	} else {
		(
			"Create a task branch using your projects",
			selectors.join(" "),
		)
	};

	let sections = [
		("Track a directory of repos", "projects track", "./repos"),
		(
			"Or track individual repos",
			"projects track",
			"./repos/api ./repos/web",
		),
		(
			instruction,
			"create",
			&format!("feat/my-feature {selectors}"),
		),
		(
			"Or check out an existing branch",
			"checkout",
			&format!("feat/my-feature {selectors}"),
		),
	];

	for (label, action, arguments) in sections {
		writeln!(terminal.output, "\n  {label}\n").expect("string output");
		terminal.command_at(
			MessageScope::Section,
			&format!("workyard {action} {arguments}"),
		);
	}

	separate(terminal);
	terminal.advice_at(
		MessageScope::Workspace,
		"If a repo has no configured default branch, add --from <branch>.",
		None,
	);
}
