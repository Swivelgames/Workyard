//! Each invocation checks consent against a fresh disclosure before unlinking worktrees.
use crate::{
	Control, Decision, DecisionKind, Participant, Problem, ProblemCode, RemovalGit,
	RemovalObservation, RemovalObserver, RemovalOptions,
};

pub(crate) async fn authorize(
	observer: &impl RemovalObserver,
	options: RemovalOptions,
	task: &str,
	participants: Option<&[String]>,
	discards_files: bool,
) -> Result<(), Problem> {
	if options.force || (options.yes && !discards_files) {
		return Ok(());
	}

	let (kind, message) = match (participants, discards_files) {
		(None, true) => (
			DecisionKind::DiscardTask,
			"Remove the task and discard the disclosed files? Discarded changes cannot be restored.".into(),
		),
		(None, false) => (
			DecisionKind::RemoveTask,
			"Remove the task worktrees while retaining branches and local commits?".into(),
		),
		(Some(names), true) => (
			DecisionKind::DiscardParticipants,
			format!("Remove {} from {task} and discard the disclosed files?", names.join(", ")),
		),
		(Some(names), false) => (
			DecisionKind::RemoveParticipants,
			format!("Remove {} from {task}?", names.join(", ")),
		),
	};
	let decision = Decision {
		task: Some(task.into()),
		kind,
		participant: String::new(),
		participants: participants.unwrap_or_default().to_vec(),
		message,
	};
	if discards_files {
		return require(observer, decision).await;
	}

	if observer.confirm(decision).await? == Some(false) {
		return Err(declined());
	}
	Ok(())
}

async fn recheck(
	git: &impl RemovalGit,
	observer: &impl RemovalObserver,
	control: &Control,
	participant: &Participant,
	disclosed: &mut RemovalObservation,
) -> Result<(), Problem> {
	loop {
		let current = git.inspect_removal(participant, control).await?;
		if !current.same_worktree(disclosed) || current.branch != disclosed.branch {
			return Err(Problem::new(
				ProblemCode::RepositoryChanged,
				"Worktree or branch changed after disclosure; inspect and retry.",
			));
		}
		let same_disclosure = if observer.requires_unchanged_disclosure() {
			current.dirty_token == disclosed.dirty_token && current.history == disclosed.history
		} else {
			current.dirty_paths.is_empty() || current.dirty_token == disclosed.dirty_token
		};
		if same_disclosure {
			*disclosed = current;
			return Ok(());
		}

		observer.disclose(&participant.name, &current).await?;
		require(
			observer,
			Decision {
				task: None,
				kind: DecisionKind::DiscardFiles,
				participant: participant.name.clone(),
				participants: Vec::new(),
				message: "Discard the newly disclosed files? These changes cannot be restored."
					.into(),
			},
		)
		.await?;
		*disclosed = current;
	}
}

/// A repository gate can outlive the first disclosure. Only start writer
/// evidence after the gate accepts the exact files authorized by the caller.
pub(crate) async fn unlink(
	git: &impl RemovalGit,
	observer: &impl RemovalObserver,
	control: &Control,
	participant: &Participant,
	disclosed: &mut RemovalObservation,
	mut before_unlink: impl FnMut(&RemovalObservation) -> Result<(), Problem> + Send,
) -> Result<(), crate::UnlinkFailure> {
	loop {
		recheck(git, observer, control, participant, disclosed).await?;
		let discard = !disclosed.dirty_paths.is_empty();
		let result = git
			.remove_worktree(disclosed, discard, control, || before_unlink(disclosed))
			.await;
		if matches!(&result, Err(crate::UnlinkFailure::BeforeLaunch(problem)) if problem.code == ProblemCode::DecisionRequired)
		{
			continue;
		}
		return result;
	}
}

pub(crate) async fn require(
	observer: &impl RemovalObserver,
	decision: Decision,
) -> Result<(), Problem> {
	let remedy = match decision.kind {
		DecisionKind::LostChanges => {
			"Review the warning, then use --yes to acknowledge the limits of undo."
		}
		_ => "Review the disclosed files. Use --force only to authorize their discard.",
	};
	let message = decision.message.clone();
	match observer.confirm(decision).await? {
		Some(true) => Ok(()),
		Some(false) => Err(declined()),
		None => Err(Problem::new(ProblemCode::DecisionRequired, message).remedy(remedy)),
	}
}

fn declined() -> Problem {
	Problem::new(
		ProblemCode::Cancelled,
		"Confirmation declined; no further worktrees will be changed.",
	)
}
