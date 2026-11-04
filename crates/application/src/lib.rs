//! Workyard orchestration and interface DTOs; Git and state remain focused adapters.
use trine::application::Application;

mod common;

// Action modules
pub mod create;
// mod list;
// mod recover;
// etc...

// Preserve the original imports while exposing capability-scoped APIs.
pub use common::{ProblemOutcome, Progress};
pub use create::{CreateInput, CreateOutcome, RepositoryInput, create};

pub fn application() -> Result<Application, Box<dyn std::error::Error>> {
	let mut app = Application::new("workyard", env!("CARGO_PKG_VERSION"))?;

	let create_entry = create::create_action()?;

	app.register(create_entry)?;

	Ok(app)
}
