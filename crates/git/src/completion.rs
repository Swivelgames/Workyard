//! Local names for advisory completion; no worktree inspection or network operations.
use crate::{Runner, process::args, repository::remote::choose_remote};
use std::{collections::BTreeSet, path::Path};
use workyard_core::{Control, Problem, ProblemCode};

pub struct Catalog {
	remotes: BTreeSet<String>,
	branches: BTreeSet<String>,
	remote_branches: BTreeSet<String>,
	tags: BTreeSet<String>,
}

impl Catalog {
	pub async fn read(path: &Path) -> Result<Self, Problem> {
		let runner = Runner::default();
		let control = Control::default();
		let remotes = runner
			.checked(path, &args(&["--no-optional-locks", "remote"]), &control)
			.await?;
		let references = runner
			.checked(
				path,
				&args(&[
					"--no-optional-locks",
					"for-each-ref",
					"--format=%(refname)%00%(symref)",
					"refs/heads/",
					"refs/remotes/",
					"refs/tags/",
				]),
				&control,
			)
			.await?;

		let mut catalog = Self {
			remotes: lines(&remotes)?.map(str::to_owned).collect(),
			branches: BTreeSet::new(),
			remote_branches: BTreeSet::new(),
			tags: BTreeSet::new(),
		};

		for line in lines(&references)? {
			let Some((reference, symbolic)) = line.split_once('\0') else {
				return Err(Problem::new(
					ProblemCode::InvalidGitOutput,
					"Malformed local reference metadata.",
				));
			};
			if !symbolic.is_empty() {
				continue;
			}

			for (prefix, values) in [
				("refs/heads/", &mut catalog.branches),
				("refs/remotes/", &mut catalog.remote_branches),
				("refs/tags/", &mut catalog.tags),
			] {
				if let Some(name) = reference.strip_prefix(prefix) {
					values.insert(name.into());
				}
			}
		}

		Ok(catalog)
	}

	pub fn remotes(&self) -> BTreeSet<String> {
		self.remotes.clone()
	}

	pub fn sources(&self) -> BTreeSet<String> {
		let mut values = self.remote_branches.clone();
		for name in &self.branches {
			let qualified = self.has_remote_prefix(name);
			values.insert(if qualified {
				format!("refs/heads/{name}")
			} else {
				name.clone()
			});
		}

		for name in &self.tags {
			let qualified = self.branches.contains(name) || self.has_remote_prefix(name);
			values.insert(if qualified {
				format!("refs/tags/{name}")
			} else {
				name.clone()
			});
		}

		values
	}

	pub fn checkout_branches(
		&self,
		requested_remote: Option<&str>,
	) -> Result<BTreeSet<String>, Problem> {
		let remotes: Vec<_> = self.remotes.iter().cloned().collect();
		let selected = choose_remote(&remotes, requested_remote)?;
		let mut values = self.branches.clone();
		if let Some(remote) = selected {
			let prefix = format!("{remote}/");
			values.extend(
				self.remote_branches
					.iter()
					.filter_map(|name| name.strip_prefix(&prefix).map(str::to_owned)),
			);
		}

		Ok(values)
	}

	fn has_remote_prefix(&self, name: &str) -> bool {
		self.remotes
			.iter()
			.any(|remote| name.starts_with(&format!("{remote}/")))
	}
}

fn lines(bytes: &[u8]) -> Result<impl Iterator<Item = &str>, Problem> {
	std::str::from_utf8(bytes).map(str::lines).map_err(|_| {
		Problem::new(
			ProblemCode::InvalidGitOutput,
			"Non-Unicode local reference metadata.",
		)
	})
}
