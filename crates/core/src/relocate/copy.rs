//! Resolve manual worktree moves and hand off Git registrations for a workspace copy.
use super::{RegisteredCopy, RelocationGit, RelocationParticipant, RelocationState};
use crate::{Control, Observer, Problem, ProblemCode};

struct Decision<'a> {
	entry: &'a RelocationParticipant,
	registration: Option<RegisteredCopy>,
	basis: Option<super::RecoveryCandidates>,
	use_registered: bool,
}

impl<'a> Decision<'a> {
	async fn inspect(
		state: &mut impl RelocationState,
		git: &impl RelocationGit,
		entry: &'a RelocationParticipant,
		undo: bool,
		control: &Control,
		observer: &impl Observer,
	) -> Result<Self, Problem> {
		let registration = registration(state, git, entry, undo, control).await?;
		let mut decision = Self {
			entry,
			registration,
			basis: None,
			use_registered: false,
		};
		let Some(registered) = &decision.registration else {
			return Ok(decision);
		};

		state.settle_recovery_direction(entry, undo, registered, control)?;
		decision.basis = state.recovery_candidates(entry, undo, registered, control)?;
		if let Some(basis) = &decision.basis {
			decision.use_registered = match &basis.first_digest {
				None => true,
				Some(first) if first == &basis.registered_digest => false,
				Some(_) => observer.choose_recovery(&basis.choice).await?,
			};
		}
		Ok(decision)
	}

	async fn is_current(
		&self,
		state: &impl RelocationState,
		git: &impl RelocationGit,
		undo: bool,
		control: &Control,
	) -> Result<bool, Problem> {
		let current = registration(state, git, self.entry, undo, control).await?;
		if current != self.registration {
			return Ok(false);
		}
		let Some(registered) = current else {
			return Ok(true);
		};
		Ok(state.recovery_candidates(self.entry, undo, &registered, control)? == self.basis)
	}

	fn record(
		self,
		state: &mut impl RelocationState,
		undo: bool,
		control: &Control,
	) -> Result<Option<(&'a RelocationParticipant, RegisteredCopy)>, Problem> {
		let Some(registered) = self.registration else {
			state.accept_registered_endpoint(self.entry, undo, control)?;
			return Ok(None);
		};
		if let Some(basis) = self.basis {
			state.select_recovery(self.entry, undo, &registered, &basis, self.use_registered)?;
		}
		Ok(Some((self.entry, registered)))
	}
}

pub(super) async fn transfer(
	state: &mut impl RelocationState,
	git: &impl RelocationGit,
	participants: &[RelocationParticipant],
	undo: bool,
	control: &Control,
	observer: &impl Observer,
) -> Result<(), Problem> {
	let recovering = state.started();
	state.begin(participants.to_vec())?;
	let displaced = if recovering {
		choose_participants(state, git, participants, undo, control, observer).await?
	} else {
		Vec::new()
	};
	state.transfer(undo, control)?;
	observer.phase("", "", "repairing_git").await?;
	for (entry, registered) in displaced {
		if registration(state, git, entry, undo, control)
			.await?
			.as_ref() != Some(&registered)
		{
			return Err(Problem::new(
				ProblemCode::RepositoryChanged,
				"Participant registration changed during workspace transfer.",
			));
		}
		state.install_recovery(entry, undo, &registered, control)?;
		state.copy_writer(entry, undo)?;
		let result = git
			.bind_displaced_copy(entry, undo, &registered, control)
			.await;
		if result
			.as_ref()
			.is_err_and(|problem| problem.code == ProblemCode::SupervisionFailed)
		{
			return result;
		}
		state.writer(false)?;
		result?;
	}
	bind_copy(state, git, participants, undo, control).await
}

async fn registration(
	state: &impl RelocationState,
	git: &impl RelocationGit,
	entry: &RelocationParticipant,
	undo: bool,
	control: &Control,
) -> Result<Option<RegisteredCopy>, Problem> {
	let previous = state.prior_registered_copy(entry)?;
	git.recovery_registration(entry, undo, previous.as_deref(), control)
		.await
}

async fn choose_participants<'a>(
	state: &mut impl RelocationState,
	git: &impl RelocationGit,
	participants: &'a [RelocationParticipant],
	undo: bool,
	control: &Control,
	observer: &impl Observer,
) -> Result<Vec<(&'a RelocationParticipant, RegisteredCopy)>, Problem> {
	'choose: loop {
		let mut decisions = Vec::new();
		for entry in participants {
			if entry.original.worktree_identity.is_none()
				|| !state.copy_participant_expected(entry)
				|| entry.original.destination == entry.destination
			{
				continue;
			}
			control.check()?;
			decisions.push(Decision::inspect(state, git, entry, undo, control, observer).await?);
		}

		// A later prompt may stay open while the user edits an earlier participant.
		// Recheck every decision before recording any of them.
		for decision in &decisions {
			control.check()?;
			if !decision.is_current(state, git, undo, control).await? {
				continue 'choose;
			}
		}

		let mut displaced = Vec::new();
		for decision in decisions {
			if let Some(entry) = decision.record(state, undo, control)? {
				displaced.push(entry);
			}
		}
		return Ok(displaced);
	}
}

async fn bind_copy(
	state: &mut impl RelocationState,
	git: &impl RelocationGit,
	participants: &[RelocationParticipant],
	undo: bool,
	control: &Control,
) -> Result<(), Problem> {
	let expected: Vec<_> = participants
		.iter()
		.filter(|entry| {
			entry.original.worktree_identity.is_some() && state.copy_participant_expected(entry)
		})
		.collect();
	for entry in &expected {
		control.check()?;
		state.verify()?;
		if git.copy_is_bound(entry, undo, control).await? {
			continue;
		}
		state.copy_writer(entry, undo)?;
		let result = git.bind_copy(entry, undo, control).await;
		if result
			.as_ref()
			.is_err_and(|problem| problem.code == ProblemCode::SupervisionFailed)
		{
			return result;
		}
		state.writer(false)?;
		result?;
	}

	for entry in expected {
		if !git.copy_is_bound(entry, undo, control).await? {
			return Err(Problem::new(
				ProblemCode::RepositoryChanged,
				format!(
					"Git registration for {} in {} changed before activation.",
					entry.original.name, entry.task
				),
			));
		}
	}
	Ok(())
}
