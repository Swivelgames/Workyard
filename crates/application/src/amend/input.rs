use crate::create::RepositoryInput;
use schemars::JsonSchema;
use serde::Deserialize;
use trine::NativePath;
use workyard_core::{AmendRequest, CreationOptions, Problem};

pub(super) struct PreparedAmendment {
	pub request: AmendRequest,
	pub root: Option<std::path::PathBuf>,
	pub state: std::path::PathBuf,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AmendInput {
	pub task: String,
	pub repositories: Vec<RepositoryInput>,
	pub tasks_root: Option<NativePath>,
	pub state_dir: Option<NativePath>,
	#[serde(default)]
	pub checkout: bool,
	#[serde(default)]
	pub no_fetch: bool,
	#[serde(default)]
	pub pull: bool,
	pub remote: Option<String>,
	pub from: Option<String>,
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
		let cwd = std::env::current_dir()
			.map_err(|error| Problem::io("Read current directory", error))?;
		let root = self.tasks_root.as_ref().map(|path| path.as_path());
		let state = workyard_state::discover_state(
			root.unwrap_or(&cwd),
			self.state_dir.as_ref().map(|path| path.as_path()),
		)?;
		let root = root.map(std::path::Path::to_path_buf);

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
			root,
			state,
		})
	}
}
