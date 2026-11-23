//! Human diagnostics from supplied failure facts.
use super::{MessageScope, Terminal, Tone, paths, text};
use crate::contracts::ProblemOutcome;
use std::{fmt::Write, path::Path};
use workyard_core::ProblemDetail;

impl Terminal {
	pub fn problem(&mut self, problem: &ProblemOutcome) {
		self.problem_at(MessageScope::Task, problem);
	}

	pub fn problem_at(&mut self, scope: MessageScope, problem: &ProblemOutcome) {
		self.problem_in_workspace(scope, problem, None);
	}

	pub fn problem_in_workspace(
		&mut self,
		scope: MessageScope,
		problem: &ProblemOutcome,
		workspace: Option<&Path>,
	) {
		if problem.code == "task_unknown" {
			let message = if problem.remedy.is_empty() {
				"No matching task in this workspace."
			} else {
				&problem.message
			};
			self.note_at(scope, message);
			let advice = match crate::guidance::commands::workspace_command("list", workspace) {
				Some(command) => format!("Run {command} to see available tasks."),
				None => "Run workyard list from the selected workspace.".into(),
			};
			self.hint_at(scope, &advice);
			return;
		}

		if let Some(placements) = &problem.worktree_locations {
			let registered = self.paths.root(placements.registered.as_path());
			let candidate = self.paths.root(placements.candidate.as_path());
			let message = if problem.code == "decision_required" {
				format!("Two worktrees share a Git registration: {registered} and {candidate}.")
			} else if placements.registered.as_path() != placements.candidate.as_path() {
				format!("Worktree moved to {candidate}. Git still points to {registered}.")
			} else {
				format!("Git link needs repair at {candidate}.")
			};
			self.note_at(scope, &message);
			self.problem_guidance(scope, &problem.remedy);
			return;
		}

		if let Some(ProblemDetail::TaskDirectoryConflict { workspace, current }) =
			problem.detail.as_deref()
		{
			let directory = self.paths.within(workspace, current);
			self.note_at(
				scope,
				&format!("The tasks directory is already set to {directory}."),
			);
			self.problem_guidance(scope, "Drop --tasks to keep the existing layout.");
			return;
		}

		let repository = problem
			.repositories
			.first()
			.map(|path| format!(" in {}", self.paths.root(path)))
			.unwrap_or_default();
		if problem.code == "git_failed"
			&& (matches!(
				problem.detail.as_deref(),
				Some(ProblemDetail::GitDiagnostic(_))
			) || problem.message.starts_with("git ["))
		{
			self.git_problem(scope, problem, &repository);
			self.hint_at(scope, "Use --json for the full Git diagnostic.");
			return;
		}

		if let Some((message, guidance)) = concise_problem(problem, &repository) {
			self.note_at(scope, &message);
			self.problem_guidance(scope, guidance);
			return;
		}
		self.explanation(scope, problem);
	}

	fn explanation(&mut self, scope: MessageScope, problem: &ProblemOutcome) {
		for line in problem
			.message
			.lines()
			.filter(|line| !line.trim().is_empty())
		{
			self.note_at(scope, line);
		}
		for candidate in &problem.candidates {
			let label = paths::display(candidate.as_path());
			self.note_at(scope, &format!("  {label}"));
		}
		self.problem_guidance(scope, &problem.remedy);
	}

	pub(super) fn git_problem(
		&mut self,
		scope: MessageScope,
		problem: &ProblemOutcome,
		repository: &str,
	) {
		self.note_at(scope, &format!("Git failed{repository}."));
		let diagnostic = match problem.detail.as_deref() {
			Some(ProblemDetail::GitDiagnostic(diagnostic)) => Some(diagnostic.as_str()),
			_ => recorded_git_detail(&problem.message),
		};
		if let Some(summary) = diagnostic.and_then(git_detail) {
			self.note_at(scope, &summary);
		}
	}

	fn problem_guidance(&mut self, scope: MessageScope, guidance: &str) {
		for line in guidance.lines().filter(|line| !line.trim().is_empty()) {
			self.hint_at(scope, line);
		}
	}

	pub fn missing_worktree(
		&mut self,
		scope: MessageScope,
		problem: &ProblemOutcome,
	) -> Option<bool> {
		let Some(ProblemDetail::MissingWorktree {
			participant,
			expected,
			occupied,
		}) = problem.detail.as_deref()
		else {
			return None;
		};

		let indent = scope.indent();
		let name = self.style.paint(&text(participant), Tone::Identity);
		let label = if *occupied {
			"path occupied"
		} else {
			"missing"
		};
		let tag = self.tag(label, Tone::Attention);
		let path = self.paths.root(expected);
		writeln!(self.output, "{indent}{name} {tag}  {path}").expect("string output");
		if *occupied {
			self.hint_at(
				scope,
				"Keep the files at this path. Verify where the original worktree moved.",
			);
		}
		Some(*occupied)
	}

	pub fn participant_problem(&mut self, participant: &str, problem: &ProblemOutcome) {
		self.note(&format!("{participant}:"));
		self.problem_at(MessageScope::Detail, problem);
	}
}

fn concise_problem(problem: &ProblemOutcome, repository: &str) -> Option<(String, &'static str)> {
	let result = match problem.code.as_str() {
		"default_branch_unknown" => (
			format!("Could not determine a starting branch{repository}."),
			"Choose a branch with --from, or --repo-from for one repo.",
		),
		"source_missing" if !repository.is_empty() => {
			let source = match problem.detail.as_deref() {
				Some(ProblemDetail::StartingPoint(source)) => format!("Starting point {source:?}"),
				_ => "The starting point".into(),
			};
			(
				format!("{source} was not found{repository}."),
				"Choose an existing branch or commit with --from or --repo-from.",
			)
		}
		"invalid_remote" if !repository.is_empty() => {
			let message = problem.message.trim_end_matches('.');
			(
				format!("{message}{repository}."),
				"Choose a configured remote with --remote or --repo-remote.",
			)
		}
		"remote_required" if !repository.is_empty() => (
			format!("Several remotes are configured{repository}."),
			"Choose a remote with --remote or --repo-remote.",
		),
		"publication_uncertain" => (
			"Could not confirm that the latest state was saved.".into(),
			"Keep existing worktrees in place. Use workyard status before retrying.",
		),
		_ => return None,
	};
	Some(result)
}

// Older task records store the invocation and stderr in a single diagnostic.
fn recorded_git_detail(message: &str) -> Option<&str> {
	let (_, exit) = message.strip_prefix("git [")?.split_once(" exited ")?;
	let (prefix, rest) = exit.split_once(": ")?;
	if matches!(prefix, "exit status" | "exit code" | "signal") {
		return rest.split_once(": ").map(|(_, diagnostic)| diagnostic);
	}
	Some(rest)
}

fn git_detail(diagnostic: &str) -> Option<String> {
	let mut lines = diagnostic
		.lines()
		.map(str::trim)
		.filter(|line| !line.is_empty());
	let first = lines.next()?;
	let detail = if first.starts_with("fatal:") || first.starts_with("error:") {
		first
	} else {
		lines
			.find(|line| line.starts_with("fatal:") || line.starts_with("error:"))
			.unwrap_or(first)
	};
	const DISPLAY_LIMIT: usize = 180;
	let mut characters = detail.chars();
	let mut summary: String = characters.by_ref().take(DISPLAY_LIMIT).collect();
	if characters.next().is_some() {
		summary.push('…');
	}
	Some(summary)
}
