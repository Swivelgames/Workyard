//! Recovery document vocabulary and validation independent of store admission.
use crate::{TaskRecord, damage::damaged};
use std::path::Path;
use workyard_core::{Participant, Phase, Problem, ProblemCode, Task, TaskStatus};

pub(crate) fn match_intent(intent: &TaskRecord, current: &TaskRecord) -> Result<(), Problem> {
	let same = intent.checkout == current.checkout
		&& intent.no_fetch == current.no_fetch
		&& intent.pull == current.pull
		&& intent.participants.len() == current.participants.len()
		&& intent
			.participants
			.iter()
			.zip(&current.participants)
			.all(|(a, b)| {
				a.name == b.name
					&& a.backing_identity == b.backing_identity
					&& a.from == b.from
					&& a.remote == b.remote
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
	if record.version != 1 {
		return Err(Problem::new(
			ProblemCode::IncompatibleState,
			"Unsupported task version in recovery evidence.",
		));
	}
	workyard_core::validate_task(&record.task)?;
	if record.task != name
		|| record.tasks_root.decode()? != root
		|| record.state_dir.decode()? != state
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
			if !path.decode()?.is_absolute() {
				return Err(damaged("Task evidence contains a relative Git path."));
			}
		}
		parse_phase(&p.phase)?;
	}
	if !matches!(
		record.status.as_str(),
		"pending" | "completed" | "failed" | "cancelled"
	) {
		return Err(damaged("Invalid task status."));
	}
	Ok(())
}

pub(crate) fn decode(record: TaskRecord) -> Result<Task, Problem> {
	Ok(Task {
		name: record.task,
		root: record.tasks_root.decode()?,
		state_dir: record.state_dir.decode()?,
		checkout: record.checkout,
		no_fetch: record.no_fetch,
		pull: record.pull,
		status: parse_status(&record.status)?,
		problem: record.problem.map(decode_problem).transpose()?,
		participants: record
			.participants
			.into_iter()
			.map(|p| {
				Ok(Participant {
					name: p.name,
					source: p.source.decode()?,
					common_dir: p.common_dir.decode()?,
					backing_identity: p.backing_identity,
					worktree_identity: p.worktree_identity,
					destination: p.destination.decode()?,
					remote: p.remote,
					from: p.from,
					phase: parse_phase(&p.phase)?,
				})
			})
			.collect::<Result<_, Problem>>()?,
	})
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

fn parse_status(value: &str) -> Result<TaskStatus, Problem> {
	match value {
		"pending" => Ok(TaskStatus::Pending),
		"completed" => Ok(TaskStatus::Completed),
		"cancelled" => Ok(TaskStatus::Cancelled),
		"failed" => Ok(TaskStatus::Failed),
		_ => Err(damaged("Invalid task status.")),
	}
}

fn decode_problem(record: crate::records::ProblemRecord) -> Result<Problem, Problem> {
	let code = record
		.code
		.parse()
		.map_err(|()| damaged(format!("Unknown stored problem code {:?}.", record.code)))?;
	Ok(Problem {
		code,
		message: record.message,
		remedy: record.remedy,
	})
}
