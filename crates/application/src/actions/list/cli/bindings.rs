use crate::presentation::cli::workspace_binding;
use trine::cli::Bindings;

pub(in super::super) fn bindings() -> Bindings {
	workspace_binding(Bindings::new())
}
