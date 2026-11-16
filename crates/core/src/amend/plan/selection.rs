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
		let existing = task
			.participants
			.iter()
			.position(|participant| participant.backing_identity == repository.identity);
		let prior = existing.map(|index| &task.participants[index]);
		let name = participant_name(input, prior)?;
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
				repair: relation.needs_repair,
				addition: false,
			});
		}

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
			repair: false,
			addition: true,
		})
	}
}

fn participant_name(
	input: &RepositoryInput,
	existing: Option<&Participant>,
) -> Result<String, Problem> {
	let name = input
		.name
		.clone()
		.or_else(|| existing.map(|participant| participant.name.clone()))
		.or_else(|| {
			input
				.path
				.file_name()
				.and_then(|name| name.to_str())
				.map(str::to_owned)
		})
		.ok_or_else(|| {
			Problem::new(
				ProblemCode::ParticipantNameRequired,
				"Supply --repo-name for a repository without a Unicode basename.",
			)
		})?;
	if !crate::safe_component(&name) {
		return Err(Problem::new(
			ProblemCode::InvalidParticipantName,
			format!("Unsafe participant name {name:?}."),
		));
	}
	Ok(name)
}

fn new_participant(
	task: &Task,
	request: &AmendRequest,
	repository: &Repository,
	name: &str,
	from: Option<String>,
) -> Participant {
	Participant {
		creation: Some(request.options),
		name: name.into(),
		source: repository.source.clone(),
		common_dir: repository.common_dir.clone(),
		backing_identity: repository.identity.clone(),
		worktree_identity: None,
		destination: task.root.join(&task.name).join(name),
		remote: repository.remote.clone(),
		from,
		phase: Phase::NotStarted,
	}
}
