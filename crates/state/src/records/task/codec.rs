//! Task document validation and decoding independent of lifecycle operations.
use crate::{TaskRecord, damage::damaged};
use std::path::Path;
use workyard_core::{Participant, Phase, Problem, ProblemCode, Task, TaskLifecycle};

pub(crate) fn match_intent(intent: &TaskRecord, current: &TaskRecord) -> Result<(), Problem> {
	let same = intent.generation == current.generation
		&& intent.checkout == current.checkout
		&& intent.no_fetch == current.no_fetch
		&& intent.pull == current.pull
		&& intent.participants.len() == current.participants.len()
		&& intent
			.participants
			.iter()
			.zip(&current.participants)
			.all(|(a, b)| {
				a.name == b.name
					&& a.registration_roots == b.registration_roots
					&& a.backing_identity == b.backing_identity
					&& a.from == b.from
					&& a.remote == b.remote
					&& a.creation == b.creation
			});
	if !same {
		return Err(damaged(
			"Task membership or options disagree with admission evidence.",
		));
	}
	Ok(())
}

pub(crate) fn validate(
	record: &TaskRecord,
	name: &str,
	root: &Path,
	state: &Path,
) -> Result<(), Problem> {
	if !matches!(record.version, 1..=4) {
		return Err(Problem::new(
			ProblemCode::IncompatibleState,
			"Unsupported task version in recovery evidence.",
		));
	}
	workyard_core::validate_task(&record.task)?;
	if record.task != name
		|| !super::placement::matches_workspace(record, root, state)?
		|| record.participants.is_empty()
		|| (record.pull && record.no_fetch)
	{
		return Err(damaged(
			"Task identity, placement or options do not match authoritative state.",
		));
	}
	let mut names = std::collections::HashSet::new();
	let mut identities = std::collections::HashSet::new();
	for p in &record.participants {
		if !workyard_core::safe_component(&p.name)
			|| !names.insert(&p.name)
			|| p.backing_identity.is_empty()
			|| !identities.insert(&p.backing_identity)
		{
			return Err(damaged("Invalid participant membership in task evidence."));
		}
		for path in [&p.source, &p.common_dir, &p.destination] {
			super::placement::check_path(&path.decode()?)?;
		}
		if p.creation
			.is_some_and(|options| options.pull && options.no_fetch)
		{
			return Err(damaged("Invalid participant creation options."));
		}
		parse_phase(&p.phase)?;
	}
	if !matches!(record.lifecycle.as_str(), "active" | "retired") {
		return Err(damaged("Invalid task lifecycle."));
	}
	if let Some(removal) = &record.removal {
		let removal = removal.decode()?;
		if removal.participants.len() != record.participants.len() {
			return Err(damaged(
				"Removal membership does not match task membership.",
			));
		}
		if record.lifecycle == "retired"
			&& (removal.retired_at.is_none()
				|| removal.direction != workyard_core::RemovalDirection::Remove
				|| removal
					.participants
					.iter()
					.any(|p| p.phase != workyard_core::RemovalPhase::Removed))
		{
			return Err(damaged("Retirement lacks complete removal evidence."));
		}
	} else if record.lifecycle == "retired" {
		return Err(damaged("Retirement lacks removal evidence."));
	}
	record.operation.decode()?;
	Ok(())
}

pub(crate) fn decode(record: TaskRecord, root: &Path, state: &Path) -> Result<Task, Problem> {
	let prior_state = record.state_dir.decode()?;
	Ok(Task {
		generation: record.generation,
		removal: record.removal.as_ref().map(|r| r.decode()).transpose()?,
		name: record.task,
		root: root.to_owned(),
		state_dir: state.to_owned(),
		checkout: record.checkout,
		no_fetch: record.no_fetch,
		pull: record.pull,
		lifecycle: if record.lifecycle == "retired" {
			TaskLifecycle::Retired
		} else {
			TaskLifecycle::Active
		},
		operation: record.operation.decode()?,
		problem: record.problem.map(decode_problem).transpose()?,
		participants: record
			.participants
			.into_iter()
			.map(|p| super::placement::participant(p, &prior_state, state))
			.collect::<Result<_, Problem>>()?,
	})
}

impl TryFrom<super::ParticipantRecord> for Participant {
	type Error = Problem;
	fn try_from(record: super::ParticipantRecord) -> Result<Self, Problem> {
		Ok(Self {
			placement: record.last_location.map(|p| p.decode()).transpose()?,
			creation: record.creation.map(Into::into),
			name: record.name,
			source: record.source.decode()?,
			common_dir: record.common_dir.decode()?,
			backing_identity: record.backing_identity,
			registration_roots: record
				.registration_roots
				.iter()
				.map(crate::path::StoredPath::registration)
				.collect::<Result<_, _>>()?,
			worktree_identity: record.worktree_identity,
			destination: record.destination.decode()?,
			remote: record.remote,
			from: record.from,
			phase: parse_phase(&record.phase)?,
		})
	}
}

fn parse_phase(value: &str) -> Result<Phase, Problem> {
	match value {
		"not_started" => Ok(Phase::NotStarted),
		"fetching" => Ok(Phase::Fetching),
		"creating" => Ok(Phase::Creating),
		"worktree_created" => Ok(Phase::WorktreeCreated),
		"pulling" => Ok(Phase::Pulling),
		"completed" => Ok(Phase::Completed),
		_ => Err(damaged("Invalid participant phase.")),
	}
}

fn decode_problem(record: crate::records::ProblemRecord) -> Result<Problem, Problem> {
	let code = record
		.code
		.parse()
		.map_err(|()| damaged(format!("Unknown stored problem code {:?}.", record.code)))?;
	Ok(Problem::new(code, record.message).remedy(record.remedy))
}
