//! Explain preflight rejections and optionally inspect an existing task.
use super::super::CreateOutcome;
use crate::{
	contracts::ProblemOutcome,
	observation::{Selection, read},
	presentation::cli::render::{MessageScope, Terminal, Tone, separate, text},
};
use std::fmt::Write;
use trine::InvocationContext;

pub(super) async fn render(
	terminal: &mut Terminal,
	outcome: &CreateOutcome,
	problem: &ProblemOutcome,
) {
	let task = &outcome.task;
	if problem.code == "task_exists" {
		existing_task(terminal, outcome).await;
		return;
	}
	if problem.code == "cancelled" {
		terminal.heading(&format!("Cancelled {task}"));
		return;
	}
	let title = terminal
		.style
		.paint(&text(&format!("Could not start {task}")), Tone::Attention);
	writeln!(terminal.output, "  {title}\n").expect("string output");

	match problem.code.as_str() {
		"branch_exists" => {
			branch_conflict(
				terminal,
				problem,
				"Branch already exists",
				"Try checkout instead",
			);
			let command = outcome
				.checkout_hint
				.clone()
				.unwrap_or_else(|| format!("workyard checkout {task} …"));
			terminal.command_at(MessageScope::Section, &command);
		}
		"branch_in_use" => {
			branch_conflict(
				terminal,
				problem,
				"Branch is already checked out",
				"Choose another branch or switch branches in the existing worktree.",
			);
			terminal.hint_at(
				MessageScope::Section,
				"If the worktree was deleted, run git worktree prune in that repo.",
			);
		}
		_ => {
			terminal.problem_at(MessageScope::Section, problem);
			if let Some(command) = &outcome.starting_branch_example {
				separate(terminal);
				terminal.note_at(
					MessageScope::Workspace,
					"For example, if your starting branch is main",
				);
				separate(terminal);
				terminal.command_at(MessageScope::Section, command);
			}
		}
	}
}

fn branch_conflict(terminal: &mut Terminal, problem: &ProblemOutcome, reason: &str, hint: &str) {
	let repositories = problem
		.repositories
		.iter()
		.map(|path| terminal.paths.root(path))
		.collect::<Vec<_>>()
		.join(", ");

	let description = if repositories.is_empty() {
		format!("{reason}.")
	} else {
		format!("{reason} in {repositories}.")
	};
	terminal.note_at(MessageScope::Section, &description);

	separate(terminal);
	terminal.heading(hint);
	separate(terminal);
}

fn task_name_in_use(terminal: &mut Terminal) {
	terminal.heading("Task name is already in use");
	separate(terminal);
	terminal.advice_at(
		MessageScope::Section,
		"Choose another name or check the task with workyard status.",
		None,
	);
}

async fn existing_task(terminal: &mut Terminal, outcome: &CreateOutcome) {
	let Some(preview) = &outcome.existing_task else {
		task_name_in_use(terminal);
		return;
	};

	let observation = read(
		Some(outcome.task.clone()),
		preview.workspace.clone(),
		Selection::Active,
		InvocationContext::unobserved(preview.cancellation.clone()),
	)
	.await;

	let observation = match observation {
		Ok(observation) if !observation.tasks.is_empty() => observation,
		Err(observation)
			if observation
				.problem
				.as_ref()
				.is_none_or(|problem| problem.code != "task_unknown") =>
		{
			task_name_in_use(terminal);
			return;
		}
		_ => {
			terminal.heading("Task directory already exists");
			separate(terminal);
			terminal.note_at(
				MessageScope::Section,
				"Choose another task name or move the existing directory.",
			);
			return;
		}
	};

	terminal.heading("Task already exists");
	let workspace = observation
		.state_dir
		.as_ref()
		.and_then(|path| path.as_path().parent());
	let tasks_root = observation.tasks_root.as_ref().map(|path| path.as_path());
	let explicit_workspace = outcome
		.explicit_workspace
		.as_ref()
		.map(|path| path.as_path());
	for task in &observation.tasks {
		separate(terminal);
		crate::presentation::cli::render::task::render(
			terminal,
			task,
			workspace,
			tasks_root,
			explicit_workspace,
		);
	}
}
