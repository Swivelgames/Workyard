pub(crate) mod completion;
pub(crate) mod exit_status;
pub(crate) mod render;

/// Every workspace-scoped action accepts the same native directory selector.
pub(crate) fn workspace_binding(bindings: trine::cli::Bindings) -> trine::cli::Bindings {
	bindings
		.option("workspace", "workspace")
		.native_path("workspace")
}
