use super::AmendInput;
use trine::NativePath;
use workyard_core::{AmendRequest, CreationOptions, Problem};

pub(super) struct PreparedAmendment {
	pub request: AmendRequest,
	pub root: Option<std::path::PathBuf>,
	pub state: std::path::PathBuf,
}

impl AmendInput {
	pub(super) fn prepare(self) -> Result<PreparedAmendment, Problem> {
		workyard_core::validate_task(&self.task)?;
		if self.repositories.is_empty() || (self.no_fetch && self.pull) {
			return Err(Problem::new(
				workyard_core::ProblemCode::InvalidInput,
				"Supply at least one --repo; --no-fetch and --pull are mutually exclusive.",
			));
		}
		let location = crate::workspace::placement::existing(
			self.tasks_root.as_ref().map(NativePath::as_path),
			self.state_dir.as_ref().map(NativePath::as_path),
		)?;

		let repositories = self
			.repositories
			.into_iter()
			.map(|repo| {
				Ok(workyard_core::RepositoryInput {
					path: repo
						.path
						.as_path()
						.canonicalize()
						.map_err(|error| Problem::io("Resolve supplied repository", error))?,
					name: repo.name,
					remote: repo.remote,
					from: repo.from,
				})
			})
			.collect::<Result<_, Problem>>()?;
		Ok(PreparedAmendment {
			request: AmendRequest {
				task: self.task,
				repositories,
				options: CreationOptions {
					checkout: self.checkout,
					no_fetch: self.no_fetch,
					pull: self.pull,
				},
				remote: self.remote,
				from: self.from,
			},
			root: location.root,
			state: location.state,
		})
	}
}
