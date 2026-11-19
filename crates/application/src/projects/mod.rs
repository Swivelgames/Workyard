//! Project selection shared by task actions, inventory and completion.
mod registration;
mod selection;
mod selector;
pub(crate) use registration::register;
pub(crate) use selection::repositories;
pub(crate) use selector::{is_explicit, resolve_selector};

use workyard_core::{Control, Problem};
use workyard_git::{Runner, projects::Discovery};
use workyard_state::Reader;

pub(crate) async fn discover(reader: &Reader, control: &Control) -> Result<Discovery, Problem> {
	Discovery::scan(
		&reader.project_roots()?,
		reader.state(),
		reader.root(),
		&Runner::default(),
		control,
	)
	.await
}
