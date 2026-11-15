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
	let mut names = HashMap::new();
	let mut participants: Vec<Participant> = Vec::new();
	let mut repositories = Vec::new();
	for input in &request.repositories {
		control.check()?;
		let remote = input.remote.as_deref().or(request.remote.as_deref());
		let repository = runner.inspect(&input.path, remote, control).await?;
		let name = input
			.name
			.clone()
			.or_else(|| {
				input
					.path
					.file_name()
					.and_then(|name| name.to_str())
					.map(str::to_owned)
			})
			.ok_or_else(|| {
				Problem::new(
					crate::ProblemCode::ParticipantNameRequired,
					"Supply --repo-name for a repository without a Unicode basename.",
				)
			})?;
		if !safe_component(&name) {
			return Err(Problem::new(
				crate::ProblemCode::InvalidParticipantName,
				format!("Unsafe participant name {name:?}."),
			));
		}

		let from = input.from.clone().or(request.from.clone());
		if let Some(index) = identities.get(&repository.common_dir) {
			let previous = &participants[*index];
			if previous.name != name
				|| previous.remote != repository.remote
				|| previous.from != from
			{
				return Err(Problem::new(
					crate::ProblemCode::RepositoryAliasConflict,
					"Repeated backing repository has conflicting names or options.",
				));
			}
			continue;
		}

		#[cfg(any(windows, target_os = "macos"))]
		let name_key = name.to_uppercase().to_lowercase();
		#[cfg(not(any(windows, target_os = "macos")))]
		let name_key = name.clone();
		if names.insert(name_key, ()).is_some() {
			return Err(Problem::new(
				crate::ProblemCode::ParticipantNameConflict,
				format!("Participant name {name:?} is not unique. Supply --repo-name."),
			));
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
		participants.push(Participant {
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
