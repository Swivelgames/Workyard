//! Keep each inspected repository with its naming and registration choices until names are final.
use crate::projects::names::{self, NameRequest};
use crate::{
	Control, CreateRequest, Git, Participant, Phase, Problem, ProblemCode, Repository,
	RepositoryInput, overlaps, safe_component,
};
use std::{
	collections::HashMap,
	path::{Path, PathBuf},
};

struct Selection<'a> {
	input_path: &'a Path,
	name: String,
	fixed_name: bool,
	from: Option<String>,
	registration_roots: Vec<PathBuf>,
	repository: Repository,
}

impl<'a> Selection<'a> {
	async fn inspect(
		input: &'a RepositoryInput,
		request: &CreateRequest,
		runner: &impl Git,
		control: &Control,
	) -> Result<Self, Problem> {
		let remote = input.remote.as_deref().or(request.remote.as_deref());
		let repository = runner.inspect(&input.path, remote, control).await?;
		input.verify_identity(&repository)?;

		let name = match &input.name {
			Some(name) => name.clone(),
			None => names::basename(&input.path)?.to_owned(),
		};
		if !safe_component(&name) {
			return Err(Problem::new(
				ProblemCode::InvalidParticipantName,
				format!("Unsafe participant name {name:?}."),
			));
		}

		Ok(Self {
			input_path: &input.path,
			name,
			fixed_name: input.name.is_some(),
			from: input.from.clone().or(request.from.clone()),
			registration_roots: input.registration_roots.clone(),
			repository,
		})
	}

	fn merge_alias(&mut self, alias: Self) -> Result<(), Problem> {
		if self.name != alias.name
			|| self.repository.remote != alias.repository.remote
			|| self.from != alias.from
		{
			return Err(Problem::new(
				ProblemCode::RepositoryAliasConflict,
				"Repeated backing repository has conflicting names or options.",
			));
		}

		self.registration_roots.extend(alias.registration_roots);
		self.fixed_name |= alias.fixed_name;
		Ok(())
	}

	fn name_request(&self) -> NameRequest<'_> {
		NameRequest {
			path: self.input_path,
			name: &self.name,
			fixed: self.fixed_name,
		}
	}

	fn into_participant(self, name: String, task_path: &Path) -> (Participant, Repository) {
		let repository = self.repository;
		let participant = Participant {
			placement: None,
			creation: None,
			registration_roots: self.registration_roots,
			worktree_identity: None,
			destination: task_path.join(&name),
			name,
			source: repository.source.clone(),
			common_dir: repository.common_dir.clone(),
			backing_identity: repository.identity.clone(),
			remote: repository.remote.clone(),
			from: self.from,
			phase: Phase::NotStarted,
		};
		(participant, repository)
	}
}

pub(super) async fn plan(
	request: &CreateRequest,
	task_path: &Path,
	runner: &impl Git,
	control: &Control,
) -> Result<(Vec<Participant>, Vec<Repository>), Problem> {
	let mut by_common_dir = HashMap::<PathBuf, usize>::new();
	let mut selected: Vec<Selection> = Vec::new();
	for input in &request.repositories {
		control.check()?;
		let selection = Selection::inspect(input, request, runner, control).await?;
		if let Some(&index) = by_common_dir.get(&selection.repository.common_dir) {
			selected[index].merge_alias(selection)?;
			continue;
		}

		let repository = &selection.repository;
		if overlaps(task_path, &repository.source) || overlaps(task_path, &repository.common_dir) {
			return Err(Problem::new(
				ProblemCode::PathOverlap,
				format!(
					"Task destination overlaps repository {:?}.",
					repository.source
				),
			));
		}

		by_common_dir.insert(repository.common_dir.clone(), selected.len());
		selected.push(selection);
	}

	let requested: Vec<_> = selected.iter().map(Selection::name_request).collect();
	let names = names::allocate(&requested, std::iter::empty())?;
	Ok(selected
		.into_iter()
		.zip(names)
		.map(|(selection, name)| selection.into_participant(name, task_path))
		.unzip())
}
