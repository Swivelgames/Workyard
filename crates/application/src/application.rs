use crate::actions::{amend, create, info, init, list, recover, remove, status};
use trine::application::Application;

pub fn application() -> Result<Application, Box<dyn std::error::Error>> {
	let mut app = Application::new("workyard", env!("CARGO_PKG_VERSION"))?;

	app.register(init::build_entry()?)?;
	app.register(create::build_entry()?)?;
	app.register(amend::build_entry()?)?;
	app.register(recover::build_entry()?)?;
	app.register(remove::build_entry()?)?;
	app.register(list::build_entry()?)?;
	app.register(info::build_entry()?)?;
	app.register(status::build_entry()?)?;

	Ok(app)
}
