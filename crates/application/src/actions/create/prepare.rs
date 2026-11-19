use super::CreateInput;
use trine::NativePath;
use workyard_core::{CreateRequest, Problem, validate_task};

impl CreateInput {
	pub fn normalize(self) -> Result<CreateRequest, Problem> {
		validate_task(&self.task)?;
		if self.repositories.is_empty() {
			return Err(Problem::new(
				workyard_core::ProblemCode::InvalidInput,
				"Supply at least one --repo.",
			));
		}

		if self.no_fetch && self.pull {
			return Err(Problem::new(
				workyard_core::ProblemCode::InvalidInput,
				"--no-fetch and --pull are mutually exclusive.",
			));
		}

		let state = crate::workspace::required(self.workspace.as_ref().map(NativePath::as_path))?;
		let reader = workyard_state::Reader::open(&state, None)?;
		Ok(CreateRequest {
			task: self.task,
			repositories: self
				.repositories
				.into_iter()
				.map(|repo| workyard_core::RepositoryInput {
					path: repo.path.into_path_buf(),
					name: repo.name,
					remote: repo.remote,
					from: repo.from,
				})
				.collect(),
			tasks_root: reader.root().to_path_buf(),
			state_dir: state,
			checkout: self.checkout,
			no_fetch: self.no_fetch,
			pull: self.pull,
			remote: self.remote,
			from: self.from,
		})
	}
}
