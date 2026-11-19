use crate::{Problem, ProblemCode};
use std::{
	collections::{BTreeMap, BTreeSet},
	path::{Path, PathBuf},
};

/// One observable path to a backing repository. Linked worktrees can share its identity.
pub struct ProjectLocation {
	pub identity: String,
	pub path: PathBuf,
}

type Suffixes = BTreeMap<PathBuf, BTreeSet<usize>>;

pub struct Project {
	pub identity: String,
	pub reference: PathBuf,
	pub path: PathBuf,
}

#[derive(Default)]
pub struct Catalog {
	projects: Vec<Project>,
	suffixes: Suffixes,
}

impl Catalog {
	pub fn new(locations: Vec<ProjectLocation>) -> Self {
		let mut aliases = BTreeMap::<String, BTreeSet<PathBuf>>::new();
		for location in locations {
			aliases
				.entry(location.identity)
				.or_default()
				.insert(location.path);
		}

		let paths: Vec<_> = aliases.into_iter().collect();
		let mut suffixes = Suffixes::new();
		for (index, (_, paths)) in paths.iter().enumerate() {
			for path in paths {
				// Index the native components so a selector never matches half a directory name.
				for suffix in suffixes_of(path) {
					suffixes.entry(suffix.to_owned()).or_default().insert(index);
				}
			}
		}

		let projects = paths
			.into_iter()
			.map(|(identity, aliases)| Project::from_aliases(identity, &aliases, &suffixes))
			.collect();
		Self { projects, suffixes }
	}

	pub fn projects(&self) -> impl Iterator<Item = &Project> {
		self.projects.iter()
	}

	/// Extend any accepted suffix, including spellings only observed through a linked clone.
	/// An ambiguous suffix offers each project's qualified reference as an actionable choice.
	pub fn completions(&self, prefix: &std::ffi::OsStr) -> Vec<(PathBuf, &Project)> {
		if prefix.is_empty() {
			return self
				.projects
				.iter()
				.map(|p| (p.reference.clone(), p))
				.collect();
		}
		let mut matches = BTreeSet::new();
		for (suffix, ids) in &self.suffixes {
			if !suffix
				.as_os_str()
				.as_encoded_bytes()
				.starts_with(prefix.as_encoded_bytes())
			{
				continue;
			}
			for &id in ids {
				let project = &self.projects[id];
				let completed = if ids.len() == 1 {
					suffix
				} else {
					&project.reference
				};
				matches.insert((completed, id));
			}
		}
		matches
			.into_iter()
			.map(|(reference, id)| (reference.clone(), &self.projects[id]))
			.collect()
	}

	pub fn resolve(&self, selector: &Path) -> Result<&Project, Problem> {
		let Some(matches) = self.suffixes.get(selector) else {
			return Err(Problem::new(ProblemCode::ProjectUnknown, format!("Unknown project {selector:?}."))
				.remedy("Run workyard projects list to see available references, or supply an explicit repository path."));
		};
		if matches.len() == 1 {
			return Ok(&self.projects[*matches.first().expect("indexed suffix")]);
		}

		let mut choices: Vec<_> = matches
			.iter()
			.map(|index| self.projects[*index].reference.clone())
			.collect();
		choices.sort();
		let mut problem = Problem::new(
			ProblemCode::ProjectAmbiguous,
			format!("Multiple projects match {selector:?}."),
		)
		.remedy("Choose a qualified project reference.");
		problem.project_remedy = Some(Box::new(crate::ProjectRemedy::Choose(choices)));
		Err(problem)
	}
}

impl Project {
	fn from_aliases(identity: String, paths: &BTreeSet<PathBuf>, suffixes: &Suffixes) -> Self {
		let choices = paths.iter().filter_map(|path| {
			let reference = suffixes_of(path).find(|suffix| suffixes[*suffix].len() == 1)?;
			Some((reference.components().count(), reference, path))
		});
		let (reference, path) = match choices.min() {
			Some((_, reference, path)) => (reference, path),
			None => {
				// Another alias contains even this path's full suffix. The absolute path still selects it.
				let path = paths.first().expect("a discovered repository has a path");
				(path.as_path(), path)
			}
		};
		Self {
			identity,
			reference: reference.to_owned(),
			path: path.to_owned(),
		}
	}
}

fn suffixes_of(path: &Path) -> impl Iterator<Item = &Path> {
	path.ancestors()
		.skip(1)
		.map(move |parent| path.strip_prefix(parent).expect("ancestor"))
}
