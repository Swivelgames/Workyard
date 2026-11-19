//! Workspace project inventory and discovery-root management.
mod contract;
mod list;
mod roots;
mod view;

use trine::{RegistrationError, application::Application};

pub(crate) fn register(app: &mut Application) -> Result<(), RegistrationError> {
	app.register(list::build_entry()?)?;
	app.register(roots::track_entry()?)?;
	app.register(roots::untrack_entry()?)
}
