//! Recovery and inspection advice for an operation already recorded in a task.
use super::{MessageScope, Terminal};
use crate::contracts::{OperationReport, ProblemOutcome};
use std::path::Path;
use workyard_core::ProblemDetail;

impl Terminal {
	pub fn recorded_task_problem(
		&mut self,
		scope: MessageScope,
		problem: &ProblemOutcome,
		task: &str,
		workspace: Option<&Path>,
		operation: &OperationReport,
	) -> bool {
		use crate::guidance::commands;
		let kind = operation.continues.or(operation.kind);
		let setup = matches!(kind, Some("create" | "amend"));
		match problem.code.as_str() {
			"git_failed" => {
				let repository = problem
					.repositories
					.first()
					.map(|path| format!(" in {}", self.paths.root(path)))
					.unwrap_or_default();
				self.git_problem(scope, problem, &repository);
				if let Some(command) = commands::status_json(task, workspace) {
					self.hint_at(scope, &format!("Git details: {command}"));
				}
			}
			"source_missing" if setup => {
				let source = match problem.detail.as_deref() {
					Some(ProblemDetail::StartingPoint(source)) => {
						format!("Starting point {source:?}")
					}
					_ => "The recorded starting point".into(),
				};
				self.note_at(scope, &format!("{source} is unavailable."));
				self.note_at(
					scope,
					"Recovery uses the recorded source. Make that source available first.",
				);
				if let Some(command) = commands::recover(task, workspace) {
					self.hint_at(scope, &format!("Then run {command}"));
				}
			}
			"branch_exists" if setup => {
				let repository = problem
					.repositories
					.first()
					.map(|path| self.paths.root(path));
				let message = match repository {
					Some(repository) => format!("The task branch already exists in {repository}."),
					None => "The task branch already exists.".into(),
				};
				self.note_at(scope, &message);
				self.note_at(
					scope,
					"This task already has a saved plan. Keep any worktrees it created.",
				);
				if kind == Some("create") {
					self.hint_at(scope, "Use a different task name to start fresh, or resolve the branch conflict before recovering this task.");
				} else {
					self.hint_at(
						scope,
						"Resolve the branch conflict before recovering this task.",
					);
				}
			}
			"branch_in_use" if setup => {
				self.note_at(scope, &problem.message);
				self.hint_at(
					scope,
					"Free the branch in the affected repository before recovering this task.",
				);
			}
			_ => return false,
		}
		true
	}
}
