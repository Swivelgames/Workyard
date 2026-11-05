//! Workyard orchestration and interface DTOs; Git and state remain focused adapters.
use trine::application::Application;

mod common;
mod invocation;
mod reporting;
mod structured;
mod terminal;

// Action modules
pub mod create;
pub mod recover;
// mod list;
// etc...

// Preserve the original imports while exposing capability-scoped APIs.
pub use common::{ProblemOutcome, Progress};
pub use create::{CreateInput, CreateOutcome, RepositoryInput, create};

pub fn application() -> Result<Application, Box<dyn std::error::Error>> {
	let mut app = Application::new("workyard", env!("CARGO_PKG_VERSION"))?;

	app.register(create::build_entry()?)?;
	app.register(recover::build_entry()?)?;

	Ok(app)
}
