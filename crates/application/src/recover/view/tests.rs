use super::*;
use std::{
	io::Write,
	path::PathBuf,
	sync::{Arc, Mutex},
};
use trine::{NativePath, application::Output};
use workyard_core::{Problem, ProblemCode};

#[derive(Clone, Default)]
struct Capture(Arc<Mutex<Vec<u8>>>);

impl Write for Capture {
	fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
		self.0.lock().unwrap().extend_from_slice(bytes);
		Ok(bytes.len())
	}

	fn flush(&mut self) -> io::Result<()> {
		Ok(())
	}
}

fn outcome(phases: &[&str]) -> RecoverOutcome {
	let mut outcome = RecoverOutcome::rejected(
		"feat/task".into(),
		Problem::new(ProblemCode::Cancelled, "cancelled"),
	);
	outcome.participants = phases
		.iter()
		.enumerate()
		.map(|(index, phase)| crate::reporting::ParticipantOutcome {
			name: format!("repo-{index}"),
			phase: (*phase).into(),
			path: NativePath::new(PathBuf::from(format!("repo-{index}"))).unwrap(),
		})
		.collect();
	outcome
}

fn render(outcome: RecoverOutcome) -> String {
	let capture = Capture::default();
	let mut view = RecoverView {
		output: CliViewContext {
			stdout: Output::new(capture.clone()),
			stderr: Output::new(io::sink()),
		},
		started: false,
	};
	smol::block_on(view.finish(&Ok(outcome))).unwrap();
	let output = String::from_utf8(capture.0.lock().unwrap().clone()).unwrap();
	for absent in [
		"Root:",
		"State:",
		"Repository",
		"[healthy]",
		"recovered",
		"\t",
	] {
		assert!(!output.contains(absent), "{output}");
	}
	output
}

#[test]
fn successful_creation_removal_and_undo_do_not_claim_historical_work_changed() {
	for phases in [
		vec!["completed"],
		vec!["removed"],
		vec!["restored", "present"],
	] {
		let mut result = outcome(&phases);
		result.operation.outcome = OperationOutcome::Succeeded;
		result.problem = None;
		assert_eq!(render(result), "Recovery succeeded: feat/task\n");
	}
}

#[test]
fn partial_recovery_reports_recorded_facts_without_claiming_current_effects() {
	for (phases, expected) in [
		(
			vec!["completed", "creating"],
			"repo-1 (creation unconfirmed)",
		),
		(
			vec!["removed", "present"],
			"repo-0 (removed), repo-1 (present)",
		),
		(
			vec!["restored", "restoration_unconfirmed"],
			"repo-0 (restored), repo-1 (restoration unconfirmed)",
		),
	] {
		let mut result = outcome(&phases);
		result.mutation_started = Some(true);
		result.operation.continues = Some("remove");
		let output = render(result);
		assert!(
			output.contains(&format!("Recorded progress: {expected}")),
			"{output}"
		);
		assert!(output.contains("Inspect: workyard status"), "{output}");
		assert!(!output.contains("No worktrees changed"), "{output}");
	}
}

#[test]
fn no_effect_decline_preserves_prior_attention_without_repeating_status() {
	let mut result = outcome(&["removed", "present"]);
	result.mutation_started = Some(false);
	result.condition = Some(crate::observation::Condition::NeedsAttention);
	let output = render(result);
	assert!(
		output.contains("Recovery cancelled: feat/task\nNo worktrees changed."),
		"{output}"
	);
	assert!(output.contains("Inspect: workyard status"), "{output}");
	assert!(!output.contains("Recorded progress"), "{output}");
	assert!(!output.contains("cancelled: cancelled"), "{output}");
}

#[test]
fn unavailable_effect_knowledge_does_not_claim_nothing_changed() {
	let mut result = outcome(&[]);
	result.operation.outcome = OperationOutcome::Failed;
	result.problem = Some(Problem::new(ProblemCode::DamagedState, "Cannot read task").into());
	let output = render(result);
	assert!(
		output.contains("damaged_state: Cannot read task"),
		"{output}"
	);
	assert!(!output.contains("No worktrees changed"), "{output}");
}
