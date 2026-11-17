//! Render already-supplied workspace guidance and problems.
use super::{MessageScope, Terminal};
use crate::contracts::ProblemOutcome;

pub(crate) fn separate(terminal: &mut Terminal) {
	if !terminal.output.is_empty() && !terminal.output.ends_with("\n\n") {
		terminal.output.push('\n');
	}
}

pub(crate) fn guidance(
	terminal: &mut Terminal,
	guidance: Option<&str>,
	problem: Option<&ProblemOutcome>,
) {
	if guidance.is_none() && problem.is_none() {
		return;
	}
	separate(terminal);
	if let Some(guidance) = guidance {
		for line in guidance.lines() {
			terminal.hint_at(MessageScope::Workspace, line);
		}
	}
	if let Some(problem) = problem {
		terminal.problem_at(MessageScope::Workspace, problem);
	}
}
