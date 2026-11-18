//! Task identity, condition and diagnostic context.
use super::participants;
use crate::observation::{Condition, Provenance, TaskReport};
use crate::presentation::cli::render::{Terminal, Tone};
use std::path::Path;
use workyard_core::OperationOutcome;

pub(crate) fn render(
	terminal: &mut Terminal,
	task: &TaskReport,
	root: Option<&Path>,
	recovery_state: Option<&Path>,
	detail: bool,
) {
	let tone = match task.condition {
		Condition::Healthy => Tone::Good,
		Condition::NeedsAttention => Tone::Attention,
		Condition::Unknown => Tone::Neutral,
	};
	let label = if task.lifecycle == Some("retired") {
		"retired"
	} else if task.provenance == Provenance::Recorded
		&& task.condition == Condition::Unknown
		&& !task.busy
		&& task.pending_participant.is_none()
	{
		"not inspected"
	} else {
		task.condition.label()
	};
	terminal.task(
		task.name.as_deref().unwrap_or("unknown identity"),
		label,
		tone,
	);

	if task.recorded.is_none() || !task.problems.is_empty() {
		let path = root.map_or_else(
			|| terminal.paths.root(task.record.as_path()),
			|root| terminal.paths.within(root, task.record.as_path()),
		);
		terminal.note(&format!("State record: {path}"));
	}

	match task.provenance {
		Provenance::Recorded => participants::recorded(terminal, task, root),
		Provenance::Inspected => {
			participants::inspected(terminal, task, root, recovery_state, detail)
		}
	}

	render_coordination(terminal, task);
	render_problems(terminal, task, detail);
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
			"{name}: a step has an unconfirmed outcome. Check that Workyard and Git have stopped before attempting recovery."
		));
	}
}

fn render_problems(terminal: &mut Terminal, task: &TaskReport, detail: bool) {
	for problem in &task.problems {
		terminal.problem(problem);
	}

	let Some(saved) = task.recorded.as_ref().or(task.evidence.as_ref()) else {
		return;
	};

	if saved.problem.is_some() || (detail && saved.operation.outcome != OperationOutcome::Succeeded)
	{
		terminal.note(&format!(
			"Recorded operation: {} ({})",
			saved.operation.kind.unwrap_or("unknown"),
			saved.operation.outcome.as_str()
		));
	}
	if let Some(problem) = &saved.problem {
		if task.condition == Condition::Healthy {
			terminal.note(&format!(
				"Historical diagnostic: {}: {}",
				problem.code, problem.message
			));
		} else {
			terminal.problem(problem);
		}
	}
}
