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
		let adding = !self.repositories.is_empty() || !self.projects.is_empty();
		if !adding && self.remove.is_empty() {
			return Err(Problem::new(
				workyard_core::ProblemCode::InvalidInput,
				"Select a project or repository to include, or name a participant to remove.",
			));
		}
		let state = crate::workspace::required(self.workspace.as_ref().map(NativePath::as_path))?;

		let repositories = if adding {
			let reader = workyard_state::Reader::open(&state, None)?;
			crate::projects::repositories(&reader, self.projects, self.repositories, control)
				.await?
		} else {
			Vec::new()
		};
		Ok(PreparedAmendment {
			request: AmendRequest {
				task: self.task,
				repositories,
				remove: self.remove,
				removal_options: workyard_core::RemovalOptions {
					yes: self.yes,
					force: self.force,
					undo: false,
				},
				options: CreationOptions {
					checkout: self.checkout,
					no_fetch: self.no_fetch,
				},
				remote: self.remote,
				from: self.from,
			},
			state,
		})
	}
}
