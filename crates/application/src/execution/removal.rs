//! Shared removal disclosure and decision adapter.
mod request;
mod review;

use crate::{
	contracts::{
		Progress,
		removal::{DecisionRequirement, Disclosure, RemovalReport},
	},
	execution::Events,
};
pub(crate) use request::{RemovalConsent, RemovalRequest};
use review::DiscardReview;
use std::{path::Path, sync::Mutex};
use trine::{InvocationContext, invocation::Confirmation};
use workyard_core::{
	AmendRequest, Decision, DecisionKind, Observer, Problem, ProblemCode, RemovalObservation,
	RemovalObserver, RetainedHistory, Task,
};

pub(crate) struct RemovalEvents {
	events: Events,
	task: String,
	report: Mutex<RemovalReport>,
	review: Option<Mutex<DiscardReview>>,
}

impl RemovalEvents {
	pub fn new(context: InvocationContext<Progress>, action: &'static str, task: String) -> Self {
		Self {
			events: Events(context, action),
			task,
			report: Mutex::default(),
			review: None,
		}
	}

	pub fn with_consent(mut self, consent: RemovalConsent, state: &Path) -> Self {
		if let RemovalConsent::ReviewDiscard { approval } = consent {
			let review = DiscardReview::new(approval, self.events.1, state);
			self.review = Some(Mutex::new(review));
		}
		self
	}

	pub fn into_report(self) -> RemovalReport {
		let mut report = self.report.into_inner().expect("removal report");
		if let Some(review) = self.review {
			if report.decision.is_some() {
				// Only outstanding worktrees belong to the next approval.
				report.disclosures.clear();
			}
			report.disclosures.extend(
				review
					.into_inner()
					.expect("discard review")
					.into_disclosures(),
			);
		}
		report
	}
}

impl Observer for RemovalEvents {
	async fn phase(&self, task: &str, participant: &str, phase: &str) -> Result<(), Problem> {
		self.events.phase(task, participant, phase).await
	}
}

impl RemovalObserver for RemovalEvents {
	async fn removed(&self, task: &str, participant: &str) -> Result<(), Problem> {
		let completed = self
			.review
			.as_ref()
			.and_then(|review| review.lock().expect("discard review").removed(participant));
		if let Some(disclosure) = completed {
			self.report
				.lock()
				.expect("removal report")
				.disclosures
				.push(disclosure);
		}
		self.phase(task, participant, "removed").await
	}

	fn begin_disclosure(&self, task: &Task, amendment: Option<&AmendRequest>) {
		if let Some(review) = &self.review {
			review
				.lock()
				.expect("discard review")
				.begin(task, amendment);
		}
	}

	fn requires_unchanged_disclosure(&self) -> bool {
		self.review
			.as_ref()
			.is_some_and(|review| review.lock().expect("discard review").has_approval())
	}

	async fn disclose(
		&self,
		participant: &str,
		observation: &RemovalObservation,
	) -> Result<(), Problem> {
		let disclosure = Disclosure {
			participant: participant.into(),
			dirty_paths: observation
				.dirty_paths
				.iter()
				.map(|path| trine::NativePath::new(path.clone()).expect("Git path"))
				.collect(),
			history: match observation.history {
				RetainedHistory::UpToDate => "up_to_date",
				RetainedHistory::Unpushed => "unpushed",
				RetainedHistory::NoUpstream => "no_upstream",
				RetainedHistory::Detached => "detached_protected",
			},
		};
		if let Some(review) = &self.review {
			review
				.lock()
				.expect("discard review")
				.observe(&disclosure, observation);
		} else {
			self.report
				.lock()
				.expect("removal report")
				.disclosures
				.push(disclosure.clone());
		}

		self.events
			.0
			.emit(Progress {
				action: self.events.1,
				task: self.task.clone(),
				participant: participant.into(),
				phase: "inspected".into(),
				disclosure: Some(disclosure),
			})
			.await
			.map_err(|_| Problem::new(ProblemCode::Cancelled, "Disclosure delivery ended."))
	}

	async fn confirm(&self, decision: Decision) -> Result<Option<bool>, Problem> {
		let kind = decision.kind;
		let mut requirement = DecisionRequirement::from(decision);
		if let Some(review) = &self.review {
			let review = review.lock().expect("discard review");
			if let Err(problem) =
				review.authorize(&mut requirement, kind == DecisionKind::DiscardFiles)
			{
				requirement.task = Some(self.task.clone());
				self.report.lock().expect("removal report").decision = Some(requirement);
				return Err(problem);
			}
			return Ok(Some(true));
		}

		if kind == DecisionKind::DiscardTask {
			self.events
				.phase(&self.task, "", "disclosed_changes_will_be_discarded")
				.await?;
		}
		if kind == DecisionKind::LostChanges {
			self.events
				.phase(&self.task, "", "discarded_changes_not_restorable")
				.await?;
		}
		let message = super::confirmation::confirmation_message(&requirement);
		let answer = self
			.events
			.0
			.confirm(Confirmation {
				code: requirement.kind.clone(),
				message,
			})
			.await
			.map_err(|_| Problem::new(ProblemCode::Cancelled, "Confirmation interrupted."))?;
		if answer.is_none()
			&& !matches!(
				kind,
				DecisionKind::RetainedHistory
					| DecisionKind::RemoveTask
					| DecisionKind::RemoveParticipants
			) {
			self.report.lock().expect("removal report").decision = Some(requirement);
		}
		Ok(answer)
	}
}
