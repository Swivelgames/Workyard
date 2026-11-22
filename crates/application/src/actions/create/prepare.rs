use super::CreateInput;
use trine::NativePath;
use workyard_core::{CreateRequest, Problem, validate_task};

impl CreateInput {
	/// Resolve workspace and project selections into a core creation request.
	pub fn normalize(self) -> Result<CreateRequest, Problem> {
		smol::block_on(self.prepare(&workyard_core::Control::default()))
	}

	pub(super) async fn prepare(
		self,
		control: &workyard_core::Control,
	) -> Result<CreateRequest, Problem> {
		validate_task(&self.task)?;
		if self.repositories.is_empty() && self.projects.is_empty() {
			return Err(Problem::new(
				workyard_core::ProblemCode::InvalidInput,
				"Select at least one project or supply --repo.",
			));
		}

		let state = crate::workspace::required(self.workspace.as_ref().map(NativePath::as_path))?;
		let reader = workyard_state::Reader::open(&state, None)?;
		let repositories =
			crate::projects::repositories(&reader, self.projects, self.repositories, control)
				.await?;
		Ok(CreateRequest {
			task: self.task,
			repositories,
			tasks_root: reader.root().to_path_buf(),
			state_dir: state,
			checkout: self.checkout,
			no_fetch: self.no_fetch,
			remote: self.remote,
			from: self.from,
		})
	}
}
