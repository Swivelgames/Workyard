//! Shared removal disclosure/decision adapter, independent of command presentation.
use crate::contracts::removal::{DecisionRequirement, Disclosure, RemovalReport};

use crate::{contracts::Progress, execution::Events};
use std::sync::Mutex;
use trine::{InvocationContext, invocation::Confirmation};
use workyard_core::{
	Decision, DecisionKind, Observer, Problem, ProblemCode, RemovalObservation, RemovalObserver,
	RetainedHistory,
};

pub(crate) struct RemovalEvents {
	events: Events,
	task: String,
	disclosures: Mutex<Vec<Disclosure>>,
	decision: Mutex<Option<DecisionRequirement>>,
}

impl RemovalEvents {
	pub fn new(context: InvocationContext<Progress>, action: &'static str, task: String) -> Self {
		Self {
			events: Events(context, action),
			task,
			disclosures: Mutex::new(Vec::new()),
			decision: Mutex::new(None),
		}
	}
	pub fn into_report(self) -> RemovalReport {
		RemovalReport {
			disclosures: self.disclosures.into_inner().expect("disclosures"),
			decision: self.decision.into_inner().expect("decision"),
		}
	}
}

impl Observer for RemovalEvents {
	async fn phase(&self, task: &str, participant: &str, phase: &str) -> Result<(), Problem> {
		self.events.phase(task, participant, phase).await
	}
}

impl RemovalObserver for RemovalEvents {
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
		self.disclosures
			.lock()
			.expect("disclosures")
			.push(disclosure.clone());
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
		let code = match decision.kind {
			DecisionKind::RemoveParticipants => "remove_participants",
			DecisionKind::DiscardParticipants => "discard_participants",
			DecisionKind::RemoveTask => "remove_task",
			DecisionKind::DiscardTask => "discard_task",
			DecisionKind::DiscardFiles => "discard_files",
			DecisionKind::RetainedHistory => "retained_history",
			DecisionKind::LostChanges => "lost_changes",
		};
		let requirement = DecisionRequirement {
			task: decision.task,
			kind: code.into(),
			participants: decision.participants,
			participant: decision.participant,
			message: decision.message,
		};
		if decision.kind == DecisionKind::DiscardTask {
			self.events
				.phase(&self.task, "", "disclosed_changes_will_be_discarded")
				.await?;
		}
		if decision.kind == DecisionKind::LostChanges {
			self.events
				.phase(&self.task, "", "discarded_changes_not_restorable")
				.await?;
		}
		let message = super::confirmation::confirmation_message(&requirement);
		let answer = self
			.events
			.0
			.confirm(Confirmation {
				code: code.into(),
				message,
			})
			.await
			.map_err(|_| Problem::new(ProblemCode::Cancelled, "Confirmation interrupted."))?;
		if answer.is_none()
			&& !matches!(
				decision.kind,
				DecisionKind::RetainedHistory
					| DecisionKind::RemoveTask
					| DecisionKind::RemoveParticipants
			) {
			*self.decision.lock().expect("decision") = Some(requirement);
		}
		Ok(answer)
	}
}
