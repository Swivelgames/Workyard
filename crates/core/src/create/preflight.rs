use crate::projects::names::{self, NameRequest};
use crate::{
	Control, CreateRequest, Git, Participant, Phase, Problem, Repository, Task, overlaps,
	safe_component,
};
use std::collections::HashMap;

pub(crate) async fn preflight(
	request: &CreateRequest,
	runner: &impl Git,
	control: &Control,
) -> Result<(Task, Vec<Repository>), Problem> {
	runner
		.validate_branch(&request.tasks_root, &request.task, control)
		.await?;
	let task_path = request.tasks_root.join(&request.task);
	if overlaps(&task_path, &request.state_dir) {
		return Err(Problem::new(
			crate::ProblemCode::PathOverlap,
			"Task destination overlaps its state directory.",
		));
	}

	let mut identities = HashMap::<std::path::PathBuf, usize>::new();
	let mut naming = Vec::<(&std::path::Path, bool)>::new();
	let mut participants: Vec<Participant> = Vec::new();
	let mut repositories = Vec::new();
	for input in &request.repositories {
		control.check()?;
		let remote = input.remote.as_deref().or(request.remote.as_deref());
		let repository = runner.inspect(&input.path, remote, control).await?;
		input.verify_identity(&repository)?;
		let name = match &input.name {
			Some(name) => name.clone(),
			None => names::basename(&input.path)?.to_owned(),
		};
		if !safe_component(&name) {
			return Err(Problem::new(
				crate::ProblemCode::InvalidParticipantName,
				format!("Unsafe participant name {name:?}."),
			));
		}

		let from = input.from.clone().or(request.from.clone());
		if let Some(index) = identities.get(&repository.common_dir) {
			let previous = &mut participants[*index];
			if previous.name != name
				|| previous.remote != repository.remote
				|| previous.from != from
			{
				return Err(Problem::new(
					crate::ProblemCode::RepositoryAliasConflict,
					"Repeated backing repository has conflicting names or options.",
				));
			}
			previous
				.registration_roots
				.extend(input.registration_roots.iter().cloned());
			naming[*index].1 |= input.name.is_some();
			continue;
		}

		if overlaps(&task_path, &repository.source) || overlaps(&task_path, &repository.common_dir)
		{
			return Err(Problem::new(
				crate::ProblemCode::PathOverlap,
				format!(
					"Task destination overlaps repository {:?}.",
					repository.source
				),
			));
		}

		identities.insert(repository.common_dir.clone(), participants.len());
		naming.push((&input.path, input.name.is_some()));
		participants.push(Participant {
			placement: None,
			creation: None,
			registration_roots: input.registration_roots.clone(),
			worktree_identity: None,
			name: name.clone(),
			source: repository.source.clone(),
			common_dir: repository.common_dir.clone(),
			backing_identity: repository.identity.clone(),
			destination: task_path.join(name),
			remote: repository.remote.clone(),
			from,
			phase: Phase::NotStarted,
		});
		repositories.push(repository);
	}

	let requested = participants
		.iter()
		.zip(naming)
		.map(|(p, (path, fixed))| NameRequest {
			path,
			name: &p.name,
			fixed,
		})
		.collect::<Vec<_>>();
	let names = names::allocate(&requested, std::iter::empty())?;
	for (participant, name) in participants.iter_mut().zip(names) {
		participant.destination = task_path.join(&name);
		participant.name = name;
	}

	let task = Task {
		generation: 0,
		removal: None,
		name: request.task.clone(),
		root: request.tasks_root.clone(),
		state_dir: request.state_dir.clone(),
		participants,
		checkout: request.checkout,
		no_fetch: request.no_fetch,
		pull: request.pull,
		lifecycle: crate::TaskLifecycle::Active,
		operation: crate::Operation::create(),
		problem: None,
	};
	Ok((task, repositories))
}

pub(crate) async fn validate_plans(
	task: &Task,
	repositories: &[Repository],
	runner: &impl Git,
	control: &Control,
) -> Result<(), Problem> {
	let mut branch_conflicts = Vec::new();
	for (repository, participant) in repositories.iter().zip(&task.participants) {
		let resolution = if task.no_fetch {
			crate::SourceResolution::Cached
		} else {
			crate::SourceResolution::FetchPending
		};
		let plan = runner
			.plan(
				repository,
				&task.name,
				task.checkout,
				participant.from.as_deref(),
				resolution,
				control,
			)
			.await;
		match plan {
			Ok(_) => {}
			Err(problem) if problem.code == crate::ProblemCode::BranchExists => {
				branch_conflicts.push(problem.message)
			}
			Err(problem) => return Err(problem),
		}
	}

	if !branch_conflicts.is_empty() {
		return Err(Problem::new(
			crate::ProblemCode::BranchExists,
			branch_conflicts.join("\n"),
		)
		.remedy("Use checkout or create --checkout to permit reuse of existing branches."));
	}

	Ok(())
}
