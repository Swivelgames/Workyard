//! Render already-supplied workspace guidance and problems.
use super::{MessageScope, Terminal};
use crate::contracts::ProblemOutcome;
use std::path::Path;

pub(crate) fn separate(terminal: &mut Terminal) {
	if terminal.output.is_empty() || terminal.output.ends_with("\n\n") {
		return;
	}

	terminal.output.push('\n');
}

pub(crate) fn guidance(
	terminal: &mut Terminal,
	guidance: Option<&str>,
	problem: Option<&ProblemOutcome>,
	workspace: Option<&Path>,
) {
	if guidance.is_none() && problem.is_none() {
		return;
	}

	separate(terminal);

	if let Some(guidance) = guidance {
		for line in guidance.lines() {
			if line.is_empty() {
				separate(terminal);
			} else {
				terminal.hint_at(MessageScope::Workspace, line);
			}
		}
	}

	if let Some(problem) = problem {
		terminal.problem_in_workspace(MessageScope::Workspace, problem, workspace);
	}
}
