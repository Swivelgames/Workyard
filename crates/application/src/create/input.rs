use schemars::JsonSchema;
use serde::Deserialize;
use trine::NativePath;
use workyard_core::{CreateRequest, Problem, validate_task};
use workyard_state::canonical_future;

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RepositoryInput {
	pub path: NativePath,
	pub name: Option<String>,
	pub remote: Option<String>,
	pub from: Option<String>,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CreateInput {
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

		let cwd = std::env::current_dir().map_err(|e| Problem::io("Read current directory", e))?;
		let root = canonical_future(
			self.tasks_root
				.as_ref()
				.map_or(cwd.as_path(), |path| path.as_path()),
		)?;
		let state = canonical_future(
			&self
				.state_dir
				.map_or_else(|| root.join(".workyard"), NativePath::into_path_buf),
		)?;
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
			tasks_root: root,
			state_dir: state,
			checkout: self.checkout,
			no_fetch: self.no_fetch,
			pull: self.pull,
			remote: self.remote,
			from: self.from,
		})
	}
}
