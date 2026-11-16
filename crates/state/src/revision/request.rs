use crate::path::StoredPath;
use serde::{Deserialize, Serialize};
use workyard_core::{AmendRequest, CreationOptions, Problem, RepositoryInput};

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct RepositoryRecord {
	path: StoredPath,
	name: Option<String>,
	remote: Option<String>,
	from: Option<String>,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct RequestRecord {
	pub(super) task: String,
	repositories: Vec<RepositoryRecord>,
	checkout: bool,
	no_fetch: bool,
	pull: bool,
	remote: Option<String>,
	from: Option<String>,
}

impl From<&AmendRequest> for RequestRecord {
	fn from(request: &AmendRequest) -> Self {
		Self {
			task: request.task.clone(),
			repositories: request
				.repositories
				.iter()
				.map(|repo| RepositoryRecord {
					path: repo.path.as_path().into(),
					name: repo.name.clone(),
					remote: repo.remote.clone(),
					from: repo.from.clone(),
				})
				.collect(),
			checkout: request.options.checkout,
			no_fetch: request.options.no_fetch,
			pull: request.options.pull,
			remote: request.remote.clone(),
			from: request.from.clone(),
		}
	}
}

impl RequestRecord {
	pub(crate) fn decode(&self) -> Result<AmendRequest, Problem> {
		Ok(AmendRequest {
			task: self.task.clone(),
			repositories: self
				.repositories
				.iter()
				.map(|repo| {
					Ok(RepositoryInput {
						path: repo.path.decode()?,
						name: repo.name.clone(),
						remote: repo.remote.clone(),
						from: repo.from.clone(),
					})
				})
				.collect::<Result<_, Problem>>()?,
			options: CreationOptions {
				checkout: self.checkout,
				no_fetch: self.no_fetch,
				pull: self.pull,
			},
			remote: self.remote.clone(),
			from: self.from.clone(),
		})
	}
}
