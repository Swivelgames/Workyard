//! Workspace project inventory and discovery-root management.
mod cli;
mod contract;
mod list;
mod roots;

use trine::{RegistrationError, application::Application};

pub(crate) fn register(app: &mut Application) -> Result<(), RegistrationError> {
	app.register(list::build_entry()?)?;
	app.register(roots::track_entry()?)?;
	app.register(roots::untrack_entry()?)
}
