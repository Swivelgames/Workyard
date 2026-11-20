//! An approval identifies the same admitted operation and bytes across client connections.
mod fingerprint;

use crate::contracts::removal::{DecisionRequirement, Disclosure};
use fingerprint::Fingerprint;
use std::{collections::BTreeMap, path::Path};
use workyard_core::{AmendRequest, Problem, ProblemCode, RemovalObservation, Task};

struct ReviewedParticipant {
	disclosure: Disclosure,
	fingerprint: [u8; 32],
}

pub(super) struct DiscardReview {
	supplied: Option<String>,
	scope: Fingerprint,
	outstanding: BTreeMap<String, ReviewedParticipant>,
}

impl DiscardReview {
	pub fn new(supplied: Option<String>, action: &str, state: &Path) -> Self {
		let mut scope = Fingerprint::default();
		scope.field(b"workyard/discard/v1");
		scope.field(action.as_bytes());
		scope.field(state.as_os_str().as_encoded_bytes());
		Self {
			supplied,
			scope,
			outstanding: BTreeMap::new(),
		}
	}

	pub fn begin(&mut self, task: &Task, amendment: Option<&AmendRequest>) {
		self.scope.field(task.name.as_bytes());
		self.scope.field(&task.generation.to_le_bytes());
		if let Some(request) = amendment {
			self.scope.amendment(request);
		}
	}

	pub fn has_approval(&self) -> bool {
		self.supplied.is_some()
	}

	pub fn observe(&mut self, disclosure: &Disclosure, observation: &RemovalObservation) {
		self.outstanding.insert(
			disclosure.participant.clone(),
			ReviewedParticipant {
				disclosure: disclosure.clone(),
				fingerprint: Fingerprint::participant(disclosure, observation),
			},
		);
	}

	pub fn removed(&mut self, participant: &str) -> Option<Disclosure> {
		self.outstanding
			.remove(participant)
			.map(|entry| entry.disclosure)
	}

	pub fn into_disclosures(self) -> impl Iterator<Item = Disclosure> {
		self.outstanding.into_values().map(|entry| entry.disclosure)
	}

	pub fn authorize(
		&self,
		requirement: &mut DecisionRequirement,
		changed: bool,
	) -> Result<(), Problem> {
		let dirty = self
			.outstanding
			.values()
			.any(|entry| !entry.disclosure.dirty_paths.is_empty());
		if !dirty && !self.has_approval() {
			return Ok(());
		}

		requirement.participants = self.outstanding.keys().cloned().collect();
		if !dirty {
			requirement.message =
				"The approved disclosure is no longer current. No files require discard.".into();
			return Err(
				Problem::new(ProblemCode::DecisionRequired, &requirement.message)
					.remedy("Repeat the request without approve_discard to continue."),
			);
		}

		let token = self.token();
		if !changed && self.supplied.as_ref() == Some(&token) {
			return Ok(());
		}
		requirement.token = Some(token);
		requirement.message = "Removing these worktrees will discard the disclosed files.".into();
		Err(Problem::new(ProblemCode::DecisionRequired, &requirement.message)
			.remedy("Review the outstanding files and obtain approval. Repeat this request with decision.token as approve_discard."))
	}

	fn token(&self) -> String {
		let mut snapshot = self.scope.clone();
		for (participant, entry) in &self.outstanding {
			snapshot.field(participant.as_bytes());
			snapshot.field(&entry.fingerprint);
		}
		snapshot.into_token()
	}
}
