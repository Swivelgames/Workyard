//! One decision covers all competing registrations disclosed in this workspace.
use super::super::super::{
	RecoverInput,
	workspace::{PlacementState, WorkspaceRecovery},
};
use crate::{
	contracts::Progress,
	guidance::{
		commands::activate_workspace,
		text::{escaped, path},
	},
};
use std::fmt::Write;
use trine::{InvocationContext, NativePath, invocation::Confirmation};
use workyard_core::{Problem, ProblemCode};

pub(super) async fn choose(
	input: &RecoverInput,
	report: &WorkspaceRecovery,
	context: &InvocationContext<Progress>,
) -> Result<bool, Problem> {
	if input.activate_here {
		return Ok(true);
	}
	let Some(prompt) = confirmation(report) else {
		return Ok(false);
	};

	match context.confirm(prompt).await {
		Ok(Some(true)) => Ok(true),
		Ok(None) => Err(decision_required(input.workspace.as_ref())),
		_ => Err(Problem::new(
			ProblemCode::Cancelled,
			"Workspace recovery cancelled.",
		)),
	}
}

fn decision_required(workspace: Option<&NativePath>) -> Problem {
	let retry = activate_workspace(workspace.map(NativePath::as_path));
	let remedy = match retry {
		Some(command) => format!("To use the worktrees in this workspace, run {command}."),
		None => "Run workyard recover --activate-here from the selected workspace.".into(),
	};
	Problem::new(
		ProblemCode::DecisionRequired,
		"Several copies share Git registrations.",
	)
	.remedy(remedy)
}

fn confirmation(report: &WorkspaceRecovery) -> Option<Confirmation> {
	let mut preview = String::new();
	let mut task = None;
	for row in report
		.participants
		.iter()
		.filter(|row| row.state == PlacementState::ChoiceRequired)
	{
		if task != row.task.as_deref() {
			task = row.task.as_deref();
			writeln!(preview, "{}", escaped(task.unwrap_or_default())).expect("string");
		}
		let old = path(row.registered.as_ref()?.as_path());
		let new = path(row.destination.as_ref()?.as_path());
		writeln!(
			preview,
			"    {}: {old} -> {new}",
			escaped(row.participant.as_deref()?)
		)
		.expect("string");
	}
	if preview.is_empty() {
		return None;
	}
	preview.push_str("Use this workspace for these worktrees?");
	Some(Confirmation {
		code: "activate_here".into(),
		message: preview,
	})
}
