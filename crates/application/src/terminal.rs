//! Composable terminal fragments. Callers choose order and completion policy.
use crate::{
	common::{ProblemOutcome, Progress},
	reporting::ParticipantOutcome,
};
use std::fmt::Write;
use trine::NativePath;
use workyard_core::TaskStatus;

pub(crate) fn progress(event: &Progress) -> String {
	format!("{}: {}\n", event.participant, event.phase)
}

pub(crate) fn task(summary: &mut String, name: &str, status: TaskStatus) {
	writeln!(summary, "Task {name}: {}", status.as_str()).expect("write to string");
}

pub(crate) fn participant(summary: &mut String, participant: &ParticipantOutcome) {
	writeln!(
		summary,
		"  {}: {} ({:?})",
		participant.name,
		participant.phase,
		participant.path.as_path()
	)
	.expect("write to string");
}

pub(crate) fn problem(summary: &mut String, problem: &ProblemOutcome) {
	writeln!(
		summary,
		"{}: {}\n{}",
		problem.code, problem.message, problem.remedy
	)
	.expect("write to string");
}

pub(crate) fn state_location(summary: &mut String, record: &NativePath) {
	writeln!(summary, "State record: {:?}", record.as_path()).expect("write to string");
}

#[cfg(test)]
mod tests {
	#[test]
	fn components_preserve_task_diagnostics_and_native_path_formatting() {
		let participant = crate::reporting::ParticipantOutcome {
			name: "api".into(),
			path: trine::NativePath::new("workspace/api").unwrap(),
			phase: "completed".into(),
		};
		let problem = crate::common::ProblemOutcome {
			code: "git_failed".into(),
			message: "pull failed".into(),
			remedy: "inspect the tree".into(),
		};
		let record = trine::NativePath::new("state/task.json").unwrap();
		let event = crate::common::Progress {
			task: "feat/test".into(),
			participant: "api".into(),
			phase: "completed".into(),
		};
		assert_eq!(super::progress(&event), "api: completed\n");

		let mut summary = String::new();
		super::task(&mut summary, "feat/test", workyard_core::TaskStatus::Failed);
		super::participant(&mut summary, &participant);
		super::problem(&mut summary, &problem);
		super::state_location(&mut summary, &record);
		assert_eq!(
			summary,
			"Task feat/test: failed\n  api: completed (\"workspace/api\")\ngit_failed: pull failed\ninspect the tree\nState record: \"state/task.json\"\n"
		);
	}
}
