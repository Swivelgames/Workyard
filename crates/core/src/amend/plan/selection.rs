use super::Change;
use crate::{
	AmendGit, AmendRequest, Control, Participant, Phase, Problem, ProblemCode, Repository,
	RepositoryInput, SourceResolution, Task,
};

impl Change {
	pub(super) async fn prepare(
		request: &AmendRequest,
		task: &Task,
		input: &RepositoryInput,
		recovering: bool,
		git: &impl AmendGit,
		control: &Control,
	) -> Result<Self, Problem> {
		let repository = git
			.inspect(
				&input.path,
				input.remote.as_deref().or(request.remote.as_deref()),
				control,
			)
			.await?;
		input.verify_identity(&repository)?;
		let existing = task
			.participants
			.iter()
			.position(|participant| participant.backing_identity == repository.identity);
		let prior = existing.map(|index| &task.participants[index]);
		let fixed_name = input.name.is_some() || prior.is_some();
		let name = match input
			.name
			.as_deref()
			.or_else(|| prior.map(|p| p.name.as_str()))
		{
			Some(name) => name,
			None => crate::projects::names::basename(&input.path)?,
		}
		.to_owned();
		let from = input.from.clone().or(request.from.clone());
		let options = if recovering {
			existing.map_or(request.options, |index| task.creation_options(index))
		} else {
			request.options
		};

		let task_path = task.root.join(&task.name);
		if (existing.is_none() && crate::overlaps(&task_path, &repository.source))
			|| crate::overlaps(&task_path, &repository.common_dir)
		{
			return Err(Problem::new(
				ProblemCode::PathOverlap,
				"Task workspace overlaps its supplied backing repository.",
			));
		}

		let mut participant = prior
			.cloned()
			.unwrap_or_else(|| new_participant(task, request, &repository, &name, from.clone()));
		let addition = participant.worktree_identity.is_none()
			&& matches!(
				participant.phase,
				Phase::NotStarted | Phase::Fetching | Phase::Creating
			);
		if !addition {
			let relation = git.relationship(&repository, &participant, control).await?;
			return Ok(Self {
				repository,
				participant: relation.participant,
				existing,
				name,
				fixed_name,
				name_source: input.path.clone(),
				repair: relation.needs_repair,
				addition: false,
			});
		}

		participant
			.registration_roots
			.extend(input.registration_roots.iter().cloned());
		participant.registration_roots.sort();
		participant.registration_roots.dedup();
		participant.creation = Some(options);
		participant.from = if recovering {
			prior.map_or(from, |participant| participant.from.clone())
		} else {
			from
		};
		participant.remote = repository.remote.clone();
		participant.source = repository.source.clone();
		participant.common_dir = repository.common_dir.clone();
		participant.name = name.clone();
		participant.destination = task_path.join(&name);

		let resolution = if options.no_fetch {
			SourceResolution::Cached
		} else {
			SourceResolution::FetchPending
		};
		git.plan(
			&repository,
			&task.name,
			options.checkout,
			participant.from.as_deref(),
			resolution,
			control,
		)
		.await?;
		Ok(Self {
			repository,
			participant,
			existing,
			name,
			fixed_name,
			name_source: input.path.clone(),
			repair: false,
			addition: true,
		})
	}
}

fn new_participant(
	task: &Task,
	request: &AmendRequest,
	repository: &Repository,
	name: &str,
	from: Option<String>,
) -> Participant {
	Participant {
		placement: None,
		creation: Some(request.options),
		name: name.into(),
		source: repository.source.clone(),
		common_dir: repository.common_dir.clone(),
		backing_identity: repository.identity.clone(),
		registration_roots: Vec::new(),
		worktree_identity: None,
		destination: task.root.join(&task.name).join(name),
		remote: repository.remote.clone(),
		from,
		phase: Phase::NotStarted,
	}
}
