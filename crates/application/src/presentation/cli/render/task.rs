//! Task identity, condition and diagnostic context.
use super::participants;
use super::{MessageScope, TaskTag, Terminal};
use crate::observation::{Condition, Provenance, TaskReport};
use std::path::Path;
use workyard_core::OperationOutcome;

pub(crate) fn render(
	terminal: &mut Terminal,
	task: &TaskReport,
	workspace: Option<&Path>,
	tasks_root: Option<&Path>,
	explicit_workspace: Option<&Path>,
) {
	let tag = if task.provenance == Provenance::Recorded
		&& task.lifecycle != Some("retired")
		&& task.condition == Condition::Unknown
		&& !task.busy
		&& task.pending_participant.is_none()
	{
		TaskTag::NotInspected
	} else {
		TaskTag::observed(task)
	};
	terminal.task(task.name.as_deref().unwrap_or("unknown identity"), tag, "");

	if task.recorded.is_none() || !task.problems.is_empty() {
		let path = workspace.map_or_else(
			|| terminal.paths.root(task.record.as_path()),
			|root| terminal.paths.within(root, task.record.as_path()),
		);
		terminal.note(&format!("State record: {path}"));
	}

	match task.provenance {
		Provenance::Recorded => participants::recorded(terminal, task, workspace),
		Provenance::Inspected => {
			participants::inspected(terminal, task, workspace, tasks_root, explicit_workspace)
		}
	}

	render_coordination(terminal, task);
	render_problems(terminal, task, explicit_workspace);
}

fn render_coordination(terminal: &mut Terminal, task: &TaskReport) {
	if task.busy {
		terminal
			.note("Task is locked; these results may change. Retry after the lock is released.");
	}
	if let Some(index) = task.pending_participant {
		let name = task
			.evidence
			.as_ref()
			.and_then(|saved| saved.participants.get(index))
			.map(|participant| participant.name.clone())
			.unwrap_or_else(|| format!("index {index}"));
		terminal.note(&format!(
			"{name}: a step has an unconfirmed outcome. Check that {} and Git have stopped before attempting recovery.", crate::identity::Identity::name()
		));
	}
}

fn render_problems(terminal: &mut Terminal, task: &TaskReport, workspace: Option<&Path>) {
	for problem in &task.problems {
		terminal.problem(problem);
	}

	let Some(saved) = task.recorded.as_ref().or(task.evidence.as_ref()) else {
		return;
	};

	if task.condition == Condition::Healthy {
		return;
	}

	if saved.problem.is_some() || saved.operation.outcome != OperationOutcome::Succeeded {
		terminal.note(&format!(
			"Recorded operation: {} ({})",
			saved.operation.kind.unwrap_or("unknown"),
			saved.operation.outcome.as_str()
		));
	}
	if let Some(problem) = &saved.problem
		&& !task.name.as_deref().is_some_and(|name| {
			terminal.recorded_task_problem(
				MessageScope::Task,
				problem,
				name,
				workspace,
				&saved.operation,
			)
		}) {
		terminal.problem(problem);
	}
}
