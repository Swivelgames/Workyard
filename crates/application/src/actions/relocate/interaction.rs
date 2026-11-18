//! Present participant recovery choices and retain unanswered decisions for API callers.
use super::contract::RecoveryCandidates;
use crate::{
	contracts::Progress,
	execution::Events,
	guidance::text::{escaped, path as display},
};
use std::{path::Path, sync::OnceLock};
use trine::{InvocationContext, NativePath, invocation::Selection};
use workyard_core::{Observer, Problem, ProblemCode, RecoveryChoice};

pub(super) struct RelocationEvents {
	events: Events,
	pub pending: OnceLock<RecoveryCandidates>,
}

impl RelocationEvents {
	pub(super) fn new(context: InvocationContext<Progress>, kind: &'static str) -> Self {
		Self {
			events: Events(context, kind),
			pending: OnceLock::new(),
		}
	}

	fn unanswered(&self, choice: &RecoveryChoice) -> Problem {
		let native = |path: &Path| NativePath::new(path).expect("verified recovery path");
		let _ = self.pending.set(RecoveryCandidates {
			participant: choice.participant.clone(),
			destination: native(&choice.destination),
			destination_occupied: choice.occupied,
			backup: choice.backup.as_deref().map(native),
			local_candidate: native(&choice.first),
			registered_candidate: native(&choice.registered),
		});
		let first = display(&choice.first);
		let registered = display(&choice.registered);
		let destination = display(&choice.destination);
		Problem::new(
			ProblemCode::DecisionRequired,
			format!(
				"Choose files from {first} or {registered} for {destination} in an interactive terminal."
			),
		)
	}
}

impl Observer for RelocationEvents {
	async fn phase(&self, task: &str, participant: &str, phase: &str) -> Result<(), Problem> {
		self.events.phase(task, participant, phase).await
	}

	async fn choose_recovery(&self, choice: &RecoveryChoice) -> Result<bool, Problem> {
		let prompt = selection(choice);
		let context = &self.events.0;
		let answer = context.select(prompt).await.map_err(|_| cancelled())?;
		if context.cancellation().is_cancelled() {
			return Err(cancelled());
		}
		match answer {
			Some(0) => Ok(false),
			Some(1) => Ok(true),
			Some(_) => Err(cancelled()),
			None => Err(self.unanswered(choice)),
		}
	}
}

fn selection(choice: &RecoveryChoice) -> Selection {
	let first = display(&choice.first);
	let registered = display(&choice.registered);
	let mut options = if let Some(backup) = &choice.backup {
		vec![
			format!("Keep files at {first}"),
			format!(
				"Use files at {registered} and keep a separate backup at {}",
				display(backup)
			),
		]
	} else {
		vec![
			format!("Use files at {first}"),
			format!("Use files at {registered}"),
		]
	};
	options.push("Cancel".into());
	Selection {
		message: format!(
			"Recover {} to {}",
			escaped(&choice.participant),
			display(&choice.destination)
		),
		options,
	}
}

fn cancelled() -> Problem {
	Problem::new(ProblemCode::Cancelled, "Recovery cancelled.")
}

#[cfg(test)]
mod tests {
	use super::*;

	fn candidates(occupied: bool) -> RecoveryChoice {
		let destination = Path::new("new").join("api");
		RecoveryChoice {
			participant: "api".into(),
			first: if occupied {
				destination.clone()
			} else {
				Path::new("old").join("api")
			},
			registered: Path::new("scratch").join("api"),
			destination,
			backup: occupied.then(|| Path::new("new").join("backup")),
			occupied,
		}
	}

	#[test]
	fn choice_labels_distinguish_replacement_from_source_selection() {
		for occupied in [true, false] {
			let choice = candidates(occupied);
			let prompt = selection(&choice);
			let first = display(&choice.first);
			let registered = display(&choice.registered);
			assert_eq!(
				prompt.message,
				format!("Recover api to {}", display(&choice.destination))
			);
			let expected = if occupied {
				[
					format!("Keep files at {first}"),
					format!(
						"Use files at {registered} and keep a separate backup at {}",
						display(choice.backup.as_ref().unwrap())
					),
				]
			} else {
				[
					format!("Use files at {first}"),
					format!("Use files at {registered}"),
				]
			};
			assert_eq!(prompt.options[..2], expected);
			assert_eq!(prompt.options[2], "Cancel");
		}
	}

	#[test]
	fn unattended_recovery_returns_both_native_candidates() {
		let events =
			RelocationEvents::new(InvocationContext::unobserved(Default::default()), "recover");
		let choice = candidates(true);
		let error = smol::block_on(events.choose_recovery(&choice)).unwrap_err();
		assert_eq!(error.code, ProblemCode::DecisionRequired);
		let pending = events.pending.into_inner().unwrap();
		assert_eq!(pending.local_candidate.as_path(), choice.first);
		assert_eq!(pending.registered_candidate.as_path(), choice.registered);
		assert!(pending.destination_occupied);
	}
}
