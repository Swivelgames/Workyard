use super::{is_explicit, resolve_selector};
use crate::contracts::RepositoryInput;
use std::{
	collections::HashSet,
	path::{Path, PathBuf},
};
use trine::NativePath;
use workyard_core::{Control, Problem, RepositoryInput as SelectedRepo, projects::Catalog};
use workyard_git::{Runner, projects::enclosing_project};
use workyard_state::Reader;

struct Resolver<'a> {
	reader: &'a Reader,
	catalog: Catalog,
	cwd: PathBuf,
	runner: Runner,
}

impl Resolver<'_> {
	async fn repository(
		&self,
		selector: &Path,
		grouped: bool,
		control: &Control,
	) -> Result<SelectedRepo, Problem> {
		let resolved = resolve_selector(&self.catalog, &self.cwd, selector, grouped)?;
		let mut path = resolved.path;
		let mut identity = resolved.identity;
		let mut registration_roots = Vec::new();
		if identity.is_none() {
			// Git also accepts interior directories. Track their enclosing project,
			// keeping symlink spellings that give the project a local name.
			if let Some(project) = enclosing_project(&path, &self.runner, control).await? {
				path = self.reader.project_registration_path(&project.path)?;
				identity = Some(project.identity);
			}
			registration_roots.push(path.clone());
		}
		Ok(SelectedRepo {
			path,
			expected_identity: identity,
			registration_roots,
			name: None,
			remote: None,
			from: None,
		})
	}
}

pub(crate) async fn repositories(
	reader: &Reader,
	projects: Vec<NativePath>,
	groups: Vec<RepositoryInput>,
	control: &Control,
) -> Result<Vec<SelectedRepo>, Problem> {
	let needs_lookup = projects
		.iter()
		.chain(groups.iter().map(|g| &g.path))
		.any(|path| !is_explicit(path.as_path()));
	let catalog = if needs_lookup {
		super::discover(reader, control).await?.catalog
	} else {
		Catalog::default()
	};
	let cwd =
		std::env::current_dir().map_err(|error| Problem::io("Read current directory", error))?;
	let resolver = Resolver {
		reader,
		catalog,
		cwd,
		runner: Runner::default(),
	};

	let mut selected = Vec::new();
	for project in projects {
		selected.push(
			resolver
				.repository(project.as_path(), false, control)
				.await?,
		);
	}

	// The first group for a positional project supplies its overrides in place.
	// Keep repeated groups for core to validate their options and backing identity.
	let positional_count = selected.len();
	let mut refined = HashSet::new();
	for group in groups {
		let mut repo = resolver
			.repository(group.path.as_path(), true, control)
			.await?;
		repo.name = group.name;
		repo.remote = group.remote;
		repo.from = group.from;
		let position = selected[..positional_count].iter().position(|p| {
			p.path == repo.path
				|| (p.expected_identity.is_some() && p.expected_identity == repo.expected_identity)
		});
		if let Some(index) = position.filter(|index| refined.insert(*index)) {
			let positional = &mut selected[index];
			if let Some(expected) = positional.expected_identity.take() {
				repo.expected_identity = Some(expected);
			}
			repo.registration_roots
				.append(&mut positional.registration_roots);
			*positional = repo;
		} else {
			selected.push(repo);
		}
	}
	Ok(selected)
}
