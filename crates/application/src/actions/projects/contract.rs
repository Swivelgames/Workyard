use crate::contracts::ProblemOutcome;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use trine::NativePath;

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(super) struct ListInput {
	/// Explicit workspace directory containing .workyard.
	pub workspace: Option<NativePath>,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(super) struct RootsInput {
	/// Repositories or directories of repositories.
	pub paths: Vec<NativePath>,
	/// Explicit workspace directory containing .workyard.
	pub workspace: Option<NativePath>,
}

#[derive(Serialize)]
pub(super) struct Root {
	pub path: NativePath,
	pub status: &'static str,
	#[serde(skip_serializing_if = "Vec::is_empty")]
	pub issues: Vec<ProblemOutcome>,
}

impl From<workyard_git::projects::Coverage> for Root {
	fn from(coverage: workyard_git::projects::Coverage) -> Self {
		let status = if coverage.missing {
			"missing"
		} else if coverage.issues.is_empty() {
			"available"
		} else {
			"incomplete"
		};

		Self {
			path: NativePath::new(coverage.root).expect("root path"),
			status,
			issues: coverage.issues.into_iter().map(Into::into).collect(),
		}
	}
}

#[derive(Serialize)]
pub(super) struct Project {
	pub reference: NativePath,
	pub path: NativePath,
}

#[derive(Default, Serialize)]
pub(super) struct Inventory {
	pub roots: Vec<Root>,
	pub projects: Vec<Project>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub guidance: Option<String>,
}

impl From<workyard_git::projects::Discovery> for Inventory {
	fn from(discovery: workyard_git::projects::Discovery) -> Self {
		let roots = discovery.coverage.into_iter().map(Root::from).collect();
		let mut projects: Vec<_> = discovery
			.catalog
			.projects()
			.map(|project| Project {
				reference: NativePath::new(&project.reference).expect("project reference"),
				path: NativePath::new(&project.path).expect("project path"),
			})
			.collect();
		projects.sort_by(|a, b| a.reference.as_path().cmp(b.reference.as_path()));

		Self {
			roots,
			projects,
			guidance: None,
		}
	}
}

#[derive(Serialize)]
pub(super) struct RootChanges {
	pub changed: Vec<NativePath>,
}
