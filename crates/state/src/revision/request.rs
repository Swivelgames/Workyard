use crate::{
	path::StoredPath,
	records::registrations::{located_project, relocated_registrations},
};
use serde::{Deserialize, Serialize};
use std::{
	collections::BTreeMap,
	path::{Path, PathBuf},
};
use workyard_core::{AmendRequest, CreationOptions, Problem, RepositoryInput};

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct RepositoryRecord {
	#[serde(default, skip_serializing_if = "Vec::is_empty")]
	registration_roots: Vec<StoredPath>,
	#[serde(skip_serializing_if = "Option::is_none")]
	expected_identity: Option<String>,
	path: StoredPath,
	name: Option<String>,
	remote: Option<String>,
	from: Option<String>,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct RequestRecord {
	pub(super) task: String,
	#[serde(default, skip_serializing_if = "Vec::is_empty")]
	pub(crate) remove: Vec<super::TargetRecord>,
	workspace_at_request: StoredPath,
	repositories: Vec<RepositoryRecord>,
	checkout: bool,
	no_fetch: bool,
	#[serde(rename = "pull")]
	legacy_pull: bool,
	remote: Option<String>,
	from: Option<String>,
}

impl RequestRecord {
	pub(crate) fn new(
		request: &AmendRequest,
		targets: &[workyard_core::RemovalTarget],
		state: &Path,
	) -> Self {
		Self {
			task: request.task.clone(),
			remove: targets.iter().map(Into::into).collect(),
			workspace_at_request: state.parent().expect("workspace marker").into(),
			repositories: request
				.repositories
				.iter()
				.map(|repo| RepositoryRecord {
					registration_roots: repo
						.registration_roots
						.iter()
						.map(|p| p.as_path().into())
						.collect(),
					expected_identity: repo.expected_identity.clone(),
					path: repo.path.as_path().into(),
					name: repo.name.clone(),
					remote: repo.remote.clone(),
					from: repo.from.clone(),
				})
				.collect(),
			checkout: request.options.checkout,
			no_fetch: request.options.no_fetch,
			legacy_pull: false,
			remote: request.remote.clone(),
			from: request.from.clone(),
		}
	}

	pub(super) fn validate(&self, task: &str, state: &Path) -> Result<(), Problem> {
		let decoded = self.decode(state)?;
		if self.task != task
			|| (decoded.repositories.is_empty() && decoded.remove.is_empty())
			|| (self.no_fetch && self.legacy_pull)
			|| decoded.repositories.iter().any(|repo| {
				!repo.path.is_absolute() || repo.path.as_os_str().as_encoded_bytes().contains(&0)
			}) {
			return Err(crate::damage::damaged(
				"Invalid recorded amendment request.",
			));
		}
		Ok(())
	}

	pub(crate) fn decode(&self, state: &Path) -> Result<AmendRequest, Problem> {
		let previous = self.workspace_at_request.registration()?;
		let workspace = state.parent().expect("workspace marker");
		let path = |stored: &StoredPath| {
			Ok(located_project(
				&stored.registration()?,
				&previous,
				workspace,
			))
		};
		Ok(AmendRequest {
			task: self.task.clone(),
			remove: self
				.remove
				.iter()
				.map(|target| target.name.clone())
				.collect(),
			removal_options: Default::default(),
			repositories: self
				.repositories
				.iter()
				.map(|repo| {
					Ok(RepositoryInput {
						registration_roots: repo
							.registration_roots
							.iter()
							.map(&path)
							.collect::<Result<_, _>>()?,
						expected_identity: repo.expected_identity.clone(),
						path: path(&repo.path)?,
						name: repo.name.clone(),
						remote: repo.remote.clone(),
						from: repo.from.clone(),
					})
				})
				.collect::<Result<_, Problem>>()?,
			options: CreationOptions {
				checkout: self.checkout,
				no_fetch: self.no_fetch,
			},
			remote: self.remote.clone(),
			from: self.from.clone(),
		})
	}

	pub(crate) fn relocated_project_roots(
		&self,
		workspace: &Path,
	) -> Result<BTreeMap<PathBuf, PathBuf>, Problem> {
		let previous = self.workspace_at_request.registration()?;
		let roots = self
			.repositories
			.iter()
			.flat_map(|repo| &repo.registration_roots);
		relocated_registrations(roots, &previous, workspace)
	}
}
