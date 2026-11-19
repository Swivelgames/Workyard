//! Allocate one flat task-local name per selected identity, in input order.
use crate::{Problem, ProblemCode, safe_component};
use std::{
	collections::{HashMap, HashSet},
	path::Path,
};

pub(crate) struct NameRequest<'a> {
	pub path: &'a Path,
	pub name: &'a str,
	pub fixed: bool,
}

pub(crate) fn basename(path: &Path) -> Result<&str, Problem> {
	path.file_name()
		.and_then(|name| name.to_str())
		.ok_or_else(|| {
			Problem::new(
				ProblemCode::ParticipantNameRequired,
				"Supply --repo-name for a repository without a Unicode basename.",
			)
		})
}

pub(crate) fn allocate<'a>(
	requests: &[NameRequest<'a>],
	reserved: impl Iterator<Item = (&'a str, &'a Path)>,
) -> Result<Vec<String>, Problem> {
	let reserved: Vec<_> = reserved.collect();
	let mut fixed: HashSet<_> = reserved.iter().map(|(name, _)| name_key(name)).collect();
	let mut peers = HashMap::<String, Vec<&Path>>::new();
	for request in requests {
		if !safe_component(request.name) {
			return Err(Problem::new(
				ProblemCode::InvalidParticipantName,
				format!("Unsafe participant name {:?}.", request.name),
			));
		}
		let key = name_key(request.name);
		if request.fixed {
			fixed.insert(key);
		} else {
			peers.entry(key).or_default().push(request.path);
		}
	}

	let mut names = Vec::with_capacity(requests.len());
	for request in requests {
		let key = name_key(request.name);
		let name = if !request.fixed && (fixed.contains(&key) || peers[&key].len() > 1) {
			qualified_name(request.path, &peers[&key])?
		} else {
			request.name.to_owned()
		};
		names.push(name);
	}

	let mut used: HashMap<_, _> = reserved
		.into_iter()
		.map(|(name, path)| (name_key(name), path))
		.collect();
	for (name, request) in names.iter().zip(requests) {
		if let Some(other) = used.insert(name_key(name), request.path) {
			let taken: HashSet<_> = names
				.iter()
				.map(|n| name_key(n))
				.chain(used.keys().cloned())
				.collect();
			let suggestion = (2..)
				.map(|n| format!("{name}-{n}"))
				.find(|name| !taken.contains(&name_key(name)))
				.expect("finite task membership");
			let mut problem = Problem::new(
				ProblemCode::ParticipantNameConflict,
				format!(
					"Participant name {name:?} is shared by {other:?} and {:?}.",
					request.path
				),
			)
			.remedy("Supply a unique participant name with --repo-name.");
			problem.project_remedy = Some(Box::new(crate::ProjectRemedy::Rename {
				source: request.path.to_owned(),
				name: suggestion,
			}));
			return Err(problem);
		}
	}
	Ok(names)
}

fn qualified_name(path: &Path, siblings: &[&Path]) -> Result<String, Problem> {
	// The basename is already taken. Start with the parent and extend only on ties.
	for parent in path.ancestors().skip(2) {
		let suffix = path.strip_prefix(parent).expect("ancestor");
		if siblings.iter().filter(|p| p.ends_with(suffix)).count() != 1 {
			continue;
		}
		let components: Option<Vec<_>> = suffix.iter().map(|part| part.to_str()).collect();
		if let Some(components) = components {
			let name = components.join("_");
			if safe_component(&name) {
				return Ok(name);
			}
		}
		break;
	}
	Err(Problem::new(
		ProblemCode::ParticipantNameRequired,
		format!("Cannot derive a unique participant name for {path:?}."),
	)
	.remedy("Supply a unique name with --repo <project> --repo-name <name>."))
}

fn name_key(name: &str) -> String {
	if cfg!(any(windows, target_os = "macos")) {
		name.to_uppercase().to_lowercase()
	} else {
		name.to_owned()
	}
}
