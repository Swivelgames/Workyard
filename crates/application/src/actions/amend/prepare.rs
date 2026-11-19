use super::AmendInput;
use trine::NativePath;
use workyard_core::{AmendRequest, CreationOptions, Problem};

pub(super) struct PreparedAmendment {
	pub request: AmendRequest,
	pub state: std::path::PathBuf,
}

impl AmendInput {
	pub(super) async fn prepare(
		self,
		control: &workyard_core::Control,
	) -> Result<PreparedAmendment, Problem> {
		workyard_core::validate_task(&self.task)?;
		if (self.repositories.is_empty() && self.projects.is_empty())
			|| (self.no_fetch && self.pull)
		{
			return Err(Problem::new(
				workyard_core::ProblemCode::InvalidInput,
				"Select at least one project or supply --repo; --no-fetch and --pull are mutually exclusive.",
			));
		}
		let state = crate::workspace::required(self.workspace.as_ref().map(NativePath::as_path))?;

		let reader = workyard_state::Reader::open(&state, None)?;
		let repositories =
			crate::projects::repositories(&reader, self.projects, self.repositories, control)
				.await?;
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
			state,
		})
	}
}
